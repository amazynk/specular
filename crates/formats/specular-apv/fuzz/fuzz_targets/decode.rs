//! Fuzz the APV decoder: raw `.apv` files (access-unit sizes), the PBU list of an access
//! unit, the frame header (chroma formats, bit depths, colour, quantisation matrices, the
//! tile grid) and the tiles (tile headers, per-component sizes and QPs, the DC and AC
//! codes, dequantisation, the inverse transform). Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fn one(au: &[u8]) {
    let _ = specular_apv::access_unit(au);
    let Ok((info, _)) = specular_apv::info(au) else {
        return;
    };
    // Keep each run cheap: large frames decode the same way, slower.
    if u64::from(info.width) * u64::from(info.height) > 1 << 18 {
        return;
    }
    if let Ok(p) = specular_apv::decode(au) {
        assert_eq!(p.planes.len(), p.info.chroma.components());
        for (c, plane) in p.planes.iter().enumerate() {
            let (w, h) = p.plane_size(c);
            assert!(plane.len() >= w * h, "plane {c} shorter than its size");
            let max = (1u32 << p.info.bit_depth) - 1;
            assert!(
                plane.iter().all(|&v| u32::from(v) <= max),
                "sample above the bit depth"
            );
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if specular_apv::is_raw_file(data) {
        for (off, len) in specular_apv::raw_units(data).into_iter().take(4) {
            assert!(off + len <= data.len());
            one(&data[off..off + len]);
        }
    } else {
        one(data);
    }
});
