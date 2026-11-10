//! Fuzz JPEG XL gain maps: the `jhgm` box walk, the gain map bundle parser (sizes, the
//! alternate's ColorEncoding bundle), the map codestream and the wide (16-bit PQ / HLG)
//! decode of an HDR base. Seeds are whole files (`gainmap_*.jxl` carry a `jhgm` box); the
//! bundle parser also gets the bytes after the first `jhgm`, so mutations of the bundle reach
//! it directly. Run from this directory with
//! `cargo +nightly fuzz run gain_map corpus/gain_map -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_jxl::{parse_gain_map_bundle, GainMapBundle, JxlFile};

fn bundle(g: &GainMapBundle, body: &[u8]) {
    let parts = [g.metadata, g.colour_encoding, g.alt_icc, g.codestream];
    assert!(parts.iter().map(|p| p.len()).sum::<usize>() <= body.len());
    assert!(g.codestream.starts_with(&[0xff, 0x0a]));
    let _ = g.alt_primaries();
}

fuzz_target!(|data: &[u8]| {
    if let Ok(g) = parse_gain_map_bundle(data) {
        bundle(&g, data);
    }
    if let Some(at) = data.windows(4).position(|w| w == b"jhgm") {
        let body = &data[at + 4..];
        if let Ok(g) = parse_gain_map_bundle(body) {
            bundle(&g, body);
        }
    }

    let Ok(f) = JxlFile::open(data) else { return };
    let info = f.info();
    if let Some(Ok(g)) = f.gain_map() {
        bundle(&g, data);
        // Keep each run cheap: large maps decode the same way, slower.
        if let Ok((w, h)) = f.gain_map_size() {
            if (w as u64) * (h as u64) <= 1 << 18 {
                if let Ok(m) = f.decode_gain_map(data.len() % 4 == 1) {
                    assert_eq!((m.width, m.height), (w, h), "map size");
                    let n = w as usize * h as usize * m.layout.bytes_per_pixel();
                    assert_eq!(m.data.len(), n, "map buffer");
                }
            }
        }
    }
    if info.width as u64 * info.height as u64 > 1 << 18 || f.decode_memory(1) > 1 << 28 {
        return;
    }
    for scale in [1, 3] {
        if let Ok(d) = f.decode_hdr(scale, data.len() % 4 == 2) {
            assert_eq!(d.width, info.width.div_ceil(scale), "scaled width");
            assert_eq!(d.height, info.height.div_ceil(scale), "scaled height");
            let n = d.width as usize * d.height as usize * 8;
            assert_eq!(d.data.len(), n, "wide buffer");
        }
    }
});
