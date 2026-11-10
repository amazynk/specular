//! Fuzz GIMP XCF: the gzip / bzip2 / xz containers, the header and property lists, the layer
//! tree (groups, masks, channels, parasites), tile decoding (raw, RLE, zlib, every precision)
//! and the compositor (every layer mode), plus single-layer rendering. Run from this directory
//! with `cargo +nightly fuzz run decode corpus/decode`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_xcf::{decompress, parse, probe_file, Image, Options};

fn check(img: &Image, what: &str) {
    let n = img.width as usize * img.height as usize * img.format.bytes_per_pixel();
    assert_eq!(img.data.len(), n, "{what}: buffer size");
}

fuzz_target!(|data: &[u8]| {
    let _ = probe_file(data);
    let owned;
    let data = match decompress(data) {
        Ok(Some(d)) => {
            owned = d;
            &owned[..]
        }
        Ok(None) => data,
        Err(_) => return,
    };
    let Ok(x) = parse(data) else { return };
    let _ = (x.icc(data), x.layers().len(), x.output_format());
    let info = x.info();
    // Keep each run cheap: large canvases composite the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 18 {
        return;
    }
    let opts = Options {
        parallel: data.len() % 2 == 1,
        cancel: None,
    };
    if let Ok(img) = x.render(data, &opts) {
        assert_eq!(img.format, x.output_format(), "render format");
        assert_eq!(
            (img.width, img.height),
            (info.width, info.height),
            "render size"
        );
        check(&img, "render");
    }
    let layers = x.pixel_layers();
    let skip = data.len() % layers.len().max(1);
    for &i in layers.iter().cycle().skip(skip).take(layers.len().min(3)) {
        if let Ok(img) = x.render_layer(data, i, &opts) {
            assert_eq!(img.format, x.layer_format(), "layer format");
            check(&img, "layer");
        }
    }
});
