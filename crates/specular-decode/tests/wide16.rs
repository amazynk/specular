//! 16-bit TIFF, PGM / PPM / PAM and PSD through the registry: the session keeps the file's
//! samples (what the inspector shows as the source value) in the most compact 16-bit layout,
//! grey staying grey, at full size and at a reduced level. Fixtures are written here.

use std::sync::Arc;

use specular_core::{CancelToken, HdrEncoding, PixelLayout, Raster, Source};
use specular_decode::{DecodeRequest, Registry};

const W: u32 = 7;
const H: u32 = 5;

/// `n` samples a pixel, every one with a distinct low byte.
fn samples(n: usize) -> Vec<u16> {
    (0..W as usize * H as usize * n)
        .map(|i| ((i * 4099 + 7) & 0xffff) as u16)
        .collect()
}

fn be(s: &[u16]) -> Vec<u8> {
    s.iter().flat_map(|v| v.to_be_bytes()).collect()
}

fn pnm(header: &str, s: &[u16]) -> Vec<u8> {
    let mut f = header.as_bytes().to_vec();
    f.extend(be(s));
    f
}

/// Little-endian, uncompressed, one strip; a second / fourth sample is straight alpha.
fn tiff(spp: u16, s: &[u16]) -> Vec<u8> {
    let data: Vec<u8> = s.iter().flat_map(|v| v.to_le_bytes()).collect();
    let alpha = spp.is_multiple_of(2);
    let n = 10 + u32::from(alpha);
    // BitsPerSample fits in the entry for one or two samples, else it follows the IFD.
    let bps_at = 8 + 2 + 12 * n + 4;
    let (bps, data_at) = if spp <= 2 {
        (16 | 16 << 16, bps_at)
    } else {
        (bps_at, bps_at + 2 * u32::from(spp))
    };
    let mut e: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, W),
        (257, 4, 1, H),
        (258, 3, u32::from(spp), bps),
        (259, 3, 1, 1),
        (262, 3, 1, if spp >= 3 { 2 } else { 1 }),
        (273, 4, 1, data_at),
        (277, 3, 1, u32::from(spp)),
        (278, 4, 1, H),
        (279, 4, 1, data.len() as u32),
        (284, 3, 1, 1),
    ];
    if alpha {
        e.push((338, 3, 1, 2));
    }
    let mut f = b"II*\0\x08\0\0\0".to_vec();
    f.extend((e.len() as u16).to_le_bytes());
    for (tag, typ, count, v) in e {
        f.extend(tag.to_le_bytes());
        f.extend(typ.to_le_bytes());
        f.extend(count.to_le_bytes());
        f.extend(v.to_le_bytes());
    }
    f.extend([0; 4]);
    if spp > 2 {
        f.extend((0..spp).flat_map(|_| 16u16.to_le_bytes()));
    }
    assert_eq!(f.len() as u32, data_at);
    f.extend(data);
    f
}

/// Uncompressed PSD (`mode` 1 grey, 3 RGB) of `planes` planes from interleaved samples.
fn psd(mode: u16, planes: usize, s: &[u16]) -> Vec<u8> {
    let mut f = b"8BPS\0\x01\0\0\0\0\0\0".to_vec();
    f.extend((planes as u16).to_be_bytes());
    f.extend(H.to_be_bytes());
    f.extend(W.to_be_bytes());
    f.extend(16u16.to_be_bytes());
    f.extend(mode.to_be_bytes());
    f.extend([0; 12]); // colour mode data, resources, layers: empty
    f.extend([0, 0]); // raw
    for c in 0..planes {
        f.extend(be(&s
            .iter()
            .skip(c)
            .step_by(planes)
            .copied()
            .collect::<Vec<_>>()));
    }
    f
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
fn sixteen_bit_sources_keep_their_samples() {
    let rgba = PixelLayout::Rgba16(HdrEncoding::SRGB);
    let cases: Vec<(&str, Vec<u8>, usize, PixelLayout)> = vec![
        (
            "grey.pgm",
            pnm(&format!("P5\n{W} {H}\n65535\n"), &samples(1)),
            1,
            PixelLayout::Gray16,
        ),
        (
            "rgb.ppm",
            pnm(&format!("P6\n{W} {H}\n65535\n"), &samples(3)),
            3,
            rgba,
        ),
        (
            "ga.pam",
            pnm(
                &format!(
                    "P7\nWIDTH {W}\nHEIGHT {H}\nDEPTH 2\nMAXVAL 65535\nTUPLTYPE GRAYSCALE_ALPHA\nENDHDR\n"
                ),
                &samples(2),
            ),
            2,
            PixelLayout::GrayAlpha16,
        ),
        ("grey.tif", tiff(1, &samples(1)), 1, PixelLayout::Gray16),
        ("ga.tif", tiff(2, &samples(2)), 2, PixelLayout::GrayAlpha16),
        ("rgb.tif", tiff(3, &samples(3)), 3, rgba),
        ("rgba.tif", tiff(4, &samples(4)), 4, rgba),
        ("grey.psd", psd(1, 1, &samples(1)), 1, PixelLayout::Gray16),
        ("rgb.psd", psd(3, 3, &samples(3)), 3, rgba),
    ];
    let c = CancelToken::new();
    for (name, file, n, layout) in cases {
        let s = Registry::default()
            .open(Arc::new(Source::from_vec(name, file)))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(s.info().bit_depth, 16, "{name}");
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.layout, layout, "{name}");
        assert_eq!((r.width, r.height), (W, H), "{name}");
        // The file's samples; RGB without alpha is opaque.
        let want: Vec<u16> = samples(n)
            .chunks_exact(n)
            .flat_map(|p| {
                let mut p = p.to_vec();
                if n == 3 {
                    p.push(65535);
                }
                p
            })
            .collect();
        assert_eq!(u16s(&r), want, "{name}");
        if s.info().scales.contains(&2) {
            let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
            assert_eq!(half.layout, layout, "{name} at 1:2");
        }
    }
}

/// A 10-bit HEIC marked PQ decodes to its 10-bit planes labelled PQ, the same samples as the
/// SDR file's, at full size, at 1:2 and for a region; converted, its rows are 8 bytes a
/// pixel (`whole::interleaved` once used 4, so every row after the first started mid-row).
/// The fixture is specular-decode-host's grad10.heic (a 10-bit gradient) with its `nclx`
/// transfer set to 16.
#[test]
fn heic_hdr_rows_are_eight_bytes_a_pixel() {
    let sdr = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../specular-decode-host/tests/data/grad10.heic"
    ))
    .unwrap();
    let at = sdr.windows(8).position(|w| w == b"colrnclx").unwrap() + 8;
    // Primaries (2 bytes), then the transfer.
    assert_eq!(&sdr[at + 2..at + 4], &[0, 13], "sRGB transfer");
    let mut pq = sdr.clone();
    pq[at + 3] = 16;
    let c = CancelToken::new();
    let open = |name: &str, f: &[u8]| {
        Registry::default()
            .open(Arc::new(Source::from_vec(name, f.to_vec())))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let (s8, s16) = (open("sdr.heic", &sdr), open("pq.heic", &pq));
    let (w, h) = (s16.info().width, s16.info().height);
    assert!(h > 4 && w > 4, "{w}×{h}");
    let reqs = [
        DecodeRequest::full(1),
        DecodeRequest::full(2),
        DecodeRequest {
            region: Some(specular_decode::Rect {
                x: 1,
                y: 2,
                w: w - 3,
                h: h - 3,
            }),
            ..DecodeRequest::full(1)
        },
    ];
    for req in reqs {
        // Both come as their 10-bit planes, the PQ one labelled PQ.
        let a = s8.decode(&req, &c).unwrap();
        let b = s16.decode(&req, &c).unwrap();
        assert!(
            matches!(a.layout, PixelLayout::Yuv(e) if e.colour == HdrEncoding::SRGB && e.bits == 10),
            "{:?}",
            a.layout
        );
        assert!(
            matches!(b.layout, PixelLayout::Yuv(e) if e.colour.transfer == specular_core::Transfer::Pq),
            "{:?}",
            b.layout
        );
        assert_eq!((a.width, a.height), (b.width, b.height));
        for (p, q) in a.planes.iter().zip(&b.planes) {
            assert!(p.data == q.data, "{req:?}: the same samples");
        }
        // Converted: RGBA16 rows of 8 bytes a pixel, the same codes, the encoding attached.
        let (a, b) = (
            specular_core::yuv::to_rgb(&a).unwrap(),
            specular_core::yuv::to_rgb(&b).unwrap(),
        );
        assert!(
            matches!(b.layout, PixelLayout::Rgba16(e) if e.transfer == specular_core::Transfer::Pq),
            "{:?}",
            b.layout
        );
        let p = &b.planes[0];
        assert_eq!(p.stride, b.width as usize * 8, "{req:?}");
        assert!(p.data.len() >= p.stride * b.height as usize, "{req:?}");
        assert_eq!(u16s(&a), u16s(&b), "{req:?}");
    }
}

/// Float TIFF (here 32-bit big-endian RGB, Deflate with the floating-point predictor) is HDR:
/// linear half-float RGBA with values above 1.0 and below 0 kept, at full size and at 1:2.
/// The fixture is specular-tiff's (`x*0.05 + y*0.07 + c*0.5 - 0.3`, 37×23).
#[test]
fn float_tiff_is_linear_half_float() {
    let file = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-tiff/tests/fixtures/compression/deflate-f4-rgb-pred3-be.tif"
    ))
    .unwrap();
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("float.tif", file)))
        .unwrap();
    let c = CancelToken::new();
    for scale in [1, 2] {
        let r = s.decode(&DecodeRequest::full(scale), &c).unwrap();
        assert_eq!(
            r.layout,
            PixelLayout::Rgba16F(specular_core::Primaries::Bt709),
            "1:{scale}"
        );
        let v: Vec<f32> = u16s(&r)
            .into_iter()
            .map(|h| {
                let e = ((h >> 10) & 0x1f) as i32;
                let m = 1.0 + (h & 0x3ff) as f32 / 1024.0;
                let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
                if e == 0 {
                    0.0
                } else {
                    s * m * 2f32.powi(e - 15)
                }
            })
            .collect();
        if scale == 1 {
            // (0, 0): -0.3, 0.2, 0.7; the last pixel's blue: 36*0.05 + 22*0.07 + 0.7 = 4.04.
            assert!(
                (v[0] + 0.3).abs() < 1e-3 && (v[2] - 0.7).abs() < 1e-3,
                "{:?}",
                &v[..4]
            );
            let last = v.len() - 4;
            assert!((v[last + 2] - 4.04).abs() < 4e-3, "{}", v[last + 2]);
        }
        assert!(v.iter().any(|&x| x > 1.5), "1:{scale}: not clamped");
    }
}
