//! An X-Trans DNG (Adobe DNG Converter output of a Fujifilm RAF) through the registry: it opens
//! on its sensor data, not just its embedded thumbnail, and renders like the RAF it came from.
//! Read in place from the large corpus (`SPECULAR_LARGE_CORPUS`, else `tools/corpus/large`);
//! skipped when absent.

use std::sync::Arc;

use specular_core::{CancelToken, Raster, Source};
use specular_decode::{DecodeRequest, Registry, Session};

fn large_corpus(rel: &str) -> Option<Vec<u8>> {
    let p = std::env::var_os("SPECULAR_LARGE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/large")
        })
        .join(rel);
    std::fs::read(p).ok()
}

fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec(name, bytes)))
        .unwrap()
}

/// Mean of each RGB channel (planar 8-bit raster).
fn means(r: &Raster) -> [f64; 3] {
    assert_eq!(r.planes.len(), 3, "{:?}", r.layout);
    std::array::from_fn(|k| {
        let p = &r.planes[k];
        assert_eq!(p.bytes_per_pixel(), 1);
        let sum: u64 = (0..p.height as usize)
            .flat_map(|y| &p.data[y * p.stride..][..p.width as usize])
            .map(|&v| v as u64)
            .sum();
        sum as f64 / (p.width as f64 * p.height as f64)
    })
}

#[test]
fn xtrans_dng_opens_on_its_sensor_like_its_raf() {
    let (Some(dng), Some(raf)) = (
        large_corpus("raw/Adobe DNG Converter/Fujifilm X-T30 II/compressed_X-T30II.dng"),
        large_corpus("raw/Fujifilm/X-T30 II/compressed_X-T30II.RAF"),
    ) else {
        eprintln!("skipped: X-T30 II DNG/RAF pair not in the large corpus");
        return;
    };
    let d = open("x.dng", dng);
    let r = open("x.raf", raf);
    assert_eq!(d.colour_model(), Some("X-Trans"));
    let info = d.info();
    assert_eq!((info.width, info.height), (6240, 4160));
    assert_eq!((info.width, info.height), (r.info().width, r.info().height));
    assert!(
        info.coding.contains("(6x6)") && !info.coding.contains("embedded preview only"),
        "{}",
        info.coding
    );
    assert_eq!(d.camera().unwrap().camera, "FUJIFILM X-T30 II");
    // The DNG's 256 px thumbnail is still offered first.
    let p = d.decode_preview().unwrap().unwrap();
    assert_eq!((p.width, p.height), (256, 171));
    let c = CancelToken::new();
    let a = d.decode(&DecodeRequest::full(8), &c).unwrap();
    let b = r.decode(&DecodeRequest::full(8), &c).unwrap();
    assert_eq!((a.width, a.height), (b.width, b.height));
    // Same samples and white balance; the DNG profile and its BaselineExposure (+0.12 EV)
    // differ from the RAF's matrix: sRGB-encoded means within 8 levels.
    let (ma, mb) = (means(&a), means(&b));
    for k in 0..3 {
        assert!((ma[k] - mb[k]).abs() < 8.0, "{ma:?} vs {mb:?}");
    }
}
