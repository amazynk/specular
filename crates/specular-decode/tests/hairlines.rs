//! Lines of drawings rasterised on the CPU (GPX/KML/GeoJSON tracks, CAD, spectra) are about a
//! pixel wide per 1024 of the picture's long side, so they survive fitting, export and
//! thumbnails. At 1 pixel on the 4096-pixel stored picture they nearly vanished (the visual
//! review).

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Registry};

/// Darkened pixels down the middle column of `scale`'s decode.
fn ink_across(bytes: &[u8], name: &str, scale: u32) -> (u32, usize) {
    let s = Registry::default()
        .open(Arc::new(Source::from_vec(name, bytes.to_vec())))
        .expect("opens");
    let r = s
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .expect("decodes");
    let p = &r.planes[0];
    let x = r.width as usize / 2;
    let ink = (0..r.height as usize)
        .filter(|&y| {
            let o = y * p.stride + x * 4;
            p.data[o..o + 3].iter().any(|&c| c < 200)
        })
        .count();
    (r.width.max(r.height), ink)
}

#[test]
fn track_lines_scale_with_the_picture() {
    let geojson = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{},"geometry":{"type":"LineString",
         "coordinates":[[10.0,50.0],[11.0,50.4]]}}]}"#;
    let (long, ink) = ink_across(geojson, "track.geojson", 1);
    assert_eq!(long, 4096);
    assert!(ink >= 3, "{ink} px across at {long}");
    // At 1/8 (512 pixels) a line stays at least 1.5 pixels.
    let (_, ink) = ink_across(geojson, "track.geojson", 8);
    assert!(ink >= 1, "{ink} px across");
}
