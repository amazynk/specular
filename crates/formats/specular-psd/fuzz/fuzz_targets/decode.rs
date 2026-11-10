//! Fuzz the PSD / PSB header, colour-mode data, image resources (ICC, EXIF, XMP, IPTC,
//! thumbnail), the layer list, RLE / ZIP composites, every colour-mode conversion and the
//! layer pages (per-channel raw / RLE / ZIP / ZIP-with-prediction, masks, opacity). Run from
//! this directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_psd::{parse, probe, Options, Planes, Samples};

/// The planes cover every pixel and convert to a whole 8-bit image.
fn check(planes: &Planes, px: usize, what: &str) {
    let n = planes.plane_count() * px;
    let len = match &planes.samples {
        Samples::U8(v) => v.len(),
        Samples::U16(v) => v.len(),
        Samples::F32(v) => v.len(),
    };
    assert!(len >= n, "{what}: planes shorter than plane_count × pixels");
    let img = planes.to_rgba8();
    assert_eq!(
        img.data.len(),
        px * img.format.channels(),
        "{what}: to_rgba8 buffer size"
    );
    if let Some(w) = planes.to_wide(false) {
        assert_eq!(
            w.data.len(),
            px * w.format.bytes_per_pixel(),
            "{what}: wide size"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let Ok(p) = parse(data) else { return };
    let _ = (
        p.icc(data),
        p.xmp(data),
        p.exif(data),
        p.iptc(data),
        p.thumbnail(data),
    );
    let _ = (p.resources(), p.layers().len());
    let info = p.info();
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 20 {
        return;
    }
    let opts = Options {
        parallel: data.len() % 2 == 1,
    };
    let px = info.width as usize * info.height as usize;
    if let Ok(img) = p.decode_rgba8(data, &opts) {
        assert_eq!(
            img.data.len(),
            px * img.format.channels(),
            "rgba8 buffer size"
        );
    }
    if let Ok(planes) = p.decode_planes(data, &opts) {
        check(&planes, px, "composite");
    }
    // A few layers per input keeps runs short; which ones varies with the input.
    let layers = p.pixel_layers();
    let skip = data.len() % layers.len().max(1);
    for &i in layers.iter().cycle().skip(skip).take(layers.len().min(4)) {
        if let Ok(planes) = p.decode_layer_planes(data, i, &opts, None) {
            assert!(planes.alpha, "layer planes carry transparency");
            check(&planes, px, "layer");
        }
    }
});
