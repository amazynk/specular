//! PNG 3rd edition cICP HDR (PQ / HLG): decoded to 16-bit `Rgba16` rasters tagged with
//! the transfer and primaries, for linearisation in the shader.

use std::io::Write;
use std::sync::Arc;

use specular_core::hdr::{self, HdrEncoding, Primaries, Transfer};
use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Registry};

fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// RGB PNG (8 or 16 bits) with an optional cICP chunk.
fn png(
    w: u32,
    h: u32,
    depth: u8,
    cicp: Option<[u8; 4]>,
    px: impl Fn(u32, u32) -> [u16; 3],
) -> Vec<u8> {
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0); // filter: none
        for x in 0..w {
            for v in px(x, y) {
                if depth == 16 {
                    raw.extend_from_slice(&v.to_be_bytes());
                } else {
                    raw.push(v as u8);
                }
            }
        }
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    z.write_all(&raw).unwrap();
    let idat = z.finish().unwrap();
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[depth, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    if let Some(c) = cicp {
        chunk(&mut out, b"cICP", &c);
    }
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn decode(file: Vec<u8>, scale: u32) -> specular_core::Raster {
    let s = Registry::new()
        .open(Arc::new(Source::from_vec("hdr.png", file)))
        .unwrap();
    s.decode(&DecodeRequest::full(scale), &CancelToken::new())
        .unwrap()
}

fn sample(r: &specular_core::Raster, x: usize, y: usize) -> [u16; 4] {
    let p = &r.planes[0];
    let o = y * p.stride + x * 8;
    std::array::from_fn(|i| u16::from_ne_bytes([p.data[o + 2 * i], p.data[o + 2 * i + 1]]))
}

#[test]
fn pq_16_bit() {
    let px = |x: u32, y: u32| [(x * 1000) as u16, (y * 3000) as u16, 40_000];
    let r = decode(png(48, 16, 16, Some([9, 16, 0, 1]), px), 1);
    assert_eq!(
        r.layout,
        PixelLayout::Rgba16(HdrEncoding {
            transfer: Transfer::Pq,
            primaries: Primaries::Bt2020
        })
    );
    for (x, y) in [(0, 0), (17, 3), (47, 15)] {
        let [a, b, c] = px(x, y);
        assert_eq!(sample(&r, x as usize, y as usize), [a, b, c, 65535]);
    }
    // PQ code 40000/65535 ≈ 0.61 ≈ 245 nits ≈ 1.2× reference white.
    let lin = hdr::decode16(
        HdrEncoding {
            transfer: Transfer::Pq,
            primaries: Primaries::Bt709,
        },
        [40_000; 3],
    );
    assert!((1.0..1.5).contains(&lin[0]), "{lin:?}");
}

#[test]
fn hlg_8_bit_p3_is_widened() {
    let r = decode(
        png(16, 8, 8, Some([12, 18, 0, 1]), |x, _| {
            [x as u16 * 16, 128, 255]
        }),
        1,
    );
    assert_eq!(
        r.layout,
        PixelLayout::Rgba16(HdrEncoding {
            transfer: Transfer::Hlg,
            primaries: Primaries::DisplayP3
        })
    );
    assert_eq!(sample(&r, 3, 2), [48 * 257, 128 * 257, 65535, 65535]);
}

#[test]
fn scaled_hdr_decode_stays_16_bit() {
    let r = decode(png(64, 32, 16, Some([9, 16, 0, 1]), |_, _| [30_000; 3]), 4);
    assert_eq!((r.width, r.height), (16, 8));
    assert!(r.layout.is_hdr());
    assert_eq!(sample(&r, 5, 5)[..3], [30_000; 3]);
}

#[test]
fn sdr_cicp_and_plain_pngs_stay_sdr() {
    // cICP sRGB (transfer 13) and no cICP: 16-bit samples stay 16-bit, sRGB-coded (SDR);
    // 8-bit ones stay 8-bit.
    for cicp in [Some([1, 13, 0, 1]), None] {
        let r = decode(png(8, 8, 16, cicp, |_, _| [1000; 3]), 1);
        assert_eq!(r.layout, PixelLayout::Rgba16(HdrEncoding::SRGB), "{cicp:?}");
        assert_eq!(sample(&r, 3, 2), [1000, 1000, 1000, 65535]);
        let r = decode(png(8, 8, 8, cicp, |_, _| [100; 3]), 1);
        assert!(!r.layout.is_wide(), "{cicp:?}: {:?}", r.layout);
    }
}
