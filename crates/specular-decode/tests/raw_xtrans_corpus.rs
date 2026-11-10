//! X-Trans RAW regions through the registry on a real RAF of the large corpus (read in place,
//! never copied; skipped when the corpus is absent).

use std::sync::Arc;

use specular_core::{CancelToken, Raster, Source};
use specular_decode::{DecodeRequest, Rect, Registry, Session};

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

/// The rows of `r` (clipped to `whole`) are byte-identical in `part` and `whole`.
fn same_pixels(part: &Raster, whole: &Raster, r: Rect, what: &str) {
    let (x0, y0) = (r.x.min(whole.width), r.y.min(whole.height));
    let (x1, y1) = ((r.x + r.w).min(whole.width), (r.y + r.h).min(whole.height));
    assert_eq!(part.planes.len(), whole.planes.len(), "{what}");
    for (pi, (p, q)) in part.planes.iter().zip(&whole.planes).enumerate() {
        let bpp = q.bytes_per_pixel();
        for y in y0..y1 {
            let a = (y - part.origin.1) as usize * p.stride + (x0 - part.origin.0) as usize * bpp;
            let b = (y - whole.origin.1) as usize * q.stride + (x0 - whole.origin.0) as usize * bpp;
            let n = (x1 - x0) as usize * bpp;
            assert!(
                p.data[a..a + n] == q.data[b..b + n],
                "{what}: plane {pi} row {y}"
            );
        }
    }
}

/// X-Trans (6x6 CFA) sensor data is held like Bayer: the first 1/1 region holds it (counted in
/// `memory_bytes`, released by `trim_memory`), and regions at every level, assembled from
/// developed tiles (kept, reused while panning), before and after a trim, equal the same pixels
/// of fresh whole levels.
#[test]
fn xtrans_sensor_held_for_regions() {
    let rel = "raw/Fujifilm/X-T30 II/compressed_X-T30II.RAF";
    let Some(file) = large_corpus(rel) else {
        eprintln!("skipped: {rel} not in the large corpus");
        return;
    };
    let s = open("x.raf", file.clone());
    let fresh = open("y.raf", file);
    assert_eq!(s.colour_model(), Some("X-Trans"));
    let c = CancelToken::new();
    let at = |r: Rect, scale: u32| DecodeRequest {
        region: Some(r),
        ..DecodeRequest::full(scale)
    };
    let first = Rect {
        x: 1201,
        y: 803,
        w: 317,
        h: 211,
    };
    assert_eq!(s.memory_bytes(), 0);
    s.decode(&at(first, 1), &c).unwrap();
    let (w, h) = (s.info().width as usize, s.info().height as usize);
    // The sensor is at least 2 bytes per output pixel (the crop is inside the sensor).
    let held = s.memory_bytes();
    assert!(held >= w * h * 2, "{held}");
    for scale in [1u32, 2, 8] {
        let whole = fresh.decode(&DecodeRequest::full(scale), &c).unwrap();
        let (lw, lh) = (whole.width, whole.height);
        for r in [
            first,
            Rect {
                x: 0,
                y: 0,
                w: 41,
                h: 29,
            },
            Rect {
                x: lw.saturating_sub(37),
                y: lh.saturating_sub(23),
                w: 64,
                h: 64,
            },
        ]
        .into_iter()
        // A tile-aligned viewer request, then panning over developed tiles.
        .chain([Rect {
            x: 512,
            y: 512,
            w: 1024,
            h: 512,
        }])
        .chain((0..4).map(|k| Rect {
            x: 700 + 200 * k,
            y: 300,
            w: 600,
            h: 333,
        })) {
            let part = s.decode(&at(r, scale), &c).unwrap();
            same_pixels(&part, &whole, r, &format!("1/{scale} {r:?}"));
        }
        if scale == 1 {
            // Released on a trim (the developed tiles too) and held again, transparently, by
            // the next region.
            let all = s.memory_bytes();
            assert!(all > held, "{all}");
            assert_eq!(s.trim_memory(), all);
            assert_eq!(s.memory_bytes(), 0);
            let part = s.decode(&at(first, 1), &c).unwrap();
            assert_eq!(s.memory_bytes(), held);
            same_pixels(&part, &whole, first, "1/1 after trim");
            // A whole level (the overview) keeps the sensor while memory allows; the first
            // 1:1 region then develops from it.
            let u = open("u.raf", large_corpus(rel).unwrap());
            u.decode(&DecodeRequest::full(8), &c).unwrap();
            if specular_core::available_memory().is_some_and(|a| a > 2 << 30) {
                assert!(u.memory_bytes() >= w * h * 2, "{}", u.memory_bytes());
            }
            let part = u.decode(&at(first, 1), &c).unwrap();
            same_pixels(&part, &whole, first, "1/1 after the overview");
        }
    }
}
