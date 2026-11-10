//! Fuzz OpenRaster / Krita documents: detection, the bounded ZIP reader (whole members and
//! header prefixes), `stack.xml` / `maindoc.xml` parsing, normal-blend compositing, layer
//! pages, Krita's tiled LZF layer data and its layer-tree compositing (blend modes, masks,
//! groups). Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_ora::{Document, detect, detect_file};

fuzz_target!(|data: &[u8]| {
    let _ = detect(data);
    let _ = detect_file(data);
    let Ok(doc) = Document::open(data) else {
        return;
    };
    for e in [doc.merged(), doc.preview()].into_iter().flatten() {
        if let Ok(head) = doc.read_prefix(data, e, 4096) {
            assert!(head.len() <= 4096);
        }
        if let Ok(all) = doc.read(data, e) {
            assert_eq!(all.len() as u64, e.size);
        }
        if let Ok(Some(r)) = doc.stored_range(data, e) {
            assert!(r.end <= data.len());
        }
    }
    // Keep each run cheap: large canvases composite the same way, slower.
    if let Ok(stack) = doc.stack(data) {
        if u64::from(stack.width) * u64::from(stack.height) <= 1 << 20 {
            if let Ok(img) = doc.composite(data, &|| false) {
                assert_eq!(
                    img.pixels.len(),
                    img.width as usize * img.height as usize * 4
                );
            }
        }
    }
    if let Some((w, h)) = doc.canvas {
        if u64::from(w) * u64::from(h) <= 1 << 20 {
            for l in doc.layers.iter().take(8) {
                if let Ok(img) = doc.layer_page(data, l, &|| false) {
                    assert_eq!((img.width, img.height), (w, h));
                    assert_eq!(img.pixels.len(), w as usize * h as usize * 4);
                }
            }
            for l in doc.kra_layers.iter().take(8) {
                if let Ok(img) = doc.kra_layer_page(data, l, &|| false) {
                    assert_eq!((img.width, img.height), (w, h));
                    assert_eq!(img.format, l.output());
                    assert_eq!(img.data.len(), w as usize * h as usize * img.format.bytes());
                }
            }
            let _ = doc.kra_notes(data, None, None);
            if let Ok(img) = doc.kra_composite(data, &|| false) {
                assert_eq!((img.width, img.height), (w, h));
                assert_eq!(Some(img.format), doc.kra_output());
                assert_eq!(img.data.len(), w as usize * h as usize * img.format.bytes());
            }
        }
    }
});
