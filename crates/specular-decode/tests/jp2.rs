//! JPEG 2000 through the registry: probing, extensions, scaled / region decodes and the
//! HTJ2K (Part 15) decodes, on the specular-jp2 test files.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Format, Rect, Registry};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-jp2/tests/data")
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(data_dir().join(name)).unwrap()
}

#[test]
fn probe_picks_jpeg2000() {
    let reg = Registry::new();
    for name in [
        "rgb_rev.j2k",
        "gray_rev.jp2",
        "palette.jp2",
        "rgb_layers.jp2",
    ] {
        let d = read(name);
        let dec = reg.probe(&d[..64.min(d.len())]).expect(name);
        assert_eq!(dec.format(), Format::Jpeg2000, "{name}");
    }
    // Not JPEG 2000: a PNG signature and the JP2 signature box with a wrong byte.
    assert!(
        reg.probe(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR")
            .is_none_or(|d| d.format() != Format::Jpeg2000)
    );
    let mut bad = read("gray_rev.jp2");
    bad[5] = b'Q';
    assert!(
        reg.probe(&bad[..64])
            .is_none_or(|d| d.format() != Format::Jpeg2000)
    );
}

#[test]
fn extensions_are_registered() {
    let reg = Registry::new();
    for ext in ["jp2", "j2k", "j2c", "jpc", "jpf", "jpx", "jph", "JP2"] {
        assert!(
            reg.supports_extension(Path::new(&format!("a.{ext}"))),
            "{ext}"
        );
    }
}

#[test]
fn scaled_and_region_decodes() {
    let reg = Registry::new();
    let s = reg
        .open(Arc::new(Source::from_vec(
            "rgb_rev.j2k",
            read("rgb_rev.j2k"),
        )))
        .unwrap();
    let info = s.info().clone();
    assert_eq!(info.format, Format::Jpeg2000);
    assert!(info.fast_regions);
    let cancel = CancelToken::new();
    for scale in [1u32, 2, 4, 8] {
        let full = s.decode(&DecodeRequest::full(scale), &cancel).unwrap();
        assert_eq!(full.width, info.width.div_ceil(scale), "scale {scale}");
        assert_eq!(full.height, info.height.div_ceil(scale), "scale {scale}");
        assert_eq!(full.layout, PixelLayout::Rgb);
        let rect = Rect {
            x: full.width / 4,
            y: full.height / 3,
            w: full.width / 2,
            h: full.height / 2,
        };
        let req = DecodeRequest {
            region: Some(rect),
            parallel: false,
            ..DecodeRequest::full(scale)
        };
        let part = s.decode(&req, &cancel).unwrap();
        assert!(part.width >= rect.w && part.height >= rect.h);
        let (ox, oy) = (part.origin.0, part.origin.1);
        assert!(ox <= rect.x && oy <= rect.y);
        for p in 0..3 {
            let a = &full.planes[p];
            let b = &part.planes[p];
            for y in 0..rect.h as usize {
                let fy = rect.y as usize + y;
                let by = fy - oy as usize;
                let fx = rect.x as usize;
                let bx = fx - ox as usize;
                assert_eq!(
                    &a.data[fy * a.stride + fx..][..rect.w as usize],
                    &b.data[by * b.stride + bx..][..rect.w as usize],
                    "scale {scale} plane {p} row {y}"
                );
            }
        }
    }
}

#[test]
fn htj2k_decodes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/corpus/conformance/openjpeg-data/input/nonregression/htj2k");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("skipping: {} missing", dir.display());
        return;
    };
    let reg = Registry::new();
    let cancel = CancelToken::new();
    let mut n = 0;
    for e in rd.flatten() {
        let p = e.path();
        let s = reg
            .open_path(&p)
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let info = s.info().clone();
        assert_eq!(info.format, Format::Jpeg2000);
        for scale in [1u32, 2] {
            let r = s
                .decode(&DecodeRequest::full(scale), &cancel)
                .unwrap_or_else(|e| panic!("{} scale {scale}: {e}", p.display()));
            assert_eq!(r.width, info.width.div_ceil(scale), "{}", p.display());
            assert_eq!(r.height, info.height.div_ceil(scale), "{}", p.display());
            // Not a flat image: HT decoding produced real content.
            let d = &r.planes[0].data;
            assert!(d.iter().any(|&v| v != d[0]), "{}: flat", p.display());
        }
        n += 1;
    }
    assert!(n >= 4, "only {n} HTJ2K files");
}
