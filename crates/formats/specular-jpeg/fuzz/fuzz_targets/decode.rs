//! Fuzz every public decode entry point. Run from this directory with
//! `cargo +nightly fuzz run decode <corpus dir> -- -max_total_time=600`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_jpeg::*;

fuzz_target!(|data: &[u8]| {
    let Ok(info) = read_info(data) else { return };
    // Keep each run cheap: skip absurd dimensions (they are rejected or decoded the same way,
    // just slower).
    if info.width as u64 * info.height as u64 > 1 << 22 {
        return;
    }
    let _ = choose_scale(&info, 300, 200);
    for scale in [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth] {
        let par = if data.len().is_multiple_of(2) {
            Parallelism::Single
        } else {
            Parallelism::Threads(3)
        };
        let o = DecodeOptions {
            scale,
            region: None,
            cancel: None,
            parallelism: par,
        };
        if let Ok(d) = decode(data, &o) {
            let rgba = planes_to_rgba(&d);
            assert_eq!(rgba.len(), d.width as usize * d.height as usize * 4);
        }
    }
    let region = Region {
        x: info.width / 3,
        y: info.height / 4,
        w: info.width / 2 + 1,
        h: info.height / 3 + 1,
    };
    let o = DecodeOptions {
        scale: Scale::Half,
        region: Some(region),
        ..Default::default()
    };
    let plain = decode(data, &o);
    if let Ok(ix) = build_index(data) {
        let with = decode_with_index(data, &ix, &o);
        if let (Ok(a), Ok(b)) = (&plain, &with) {
            if !a.truncated && !b.truncated {
                assert!(a == b, "indexed region decode differs");
            }
        }
        // Threaded indexed decodes (progressive arithmetic scans run pipelined from the
        // region's checkpoints) equal the serial one, truncated or not.
        for parallelism in [Parallelism::Single, Parallelism::Threads(3)] {
            let o = DecodeOptions { parallelism, ..o };
            let t = decode_with_index(data, &ix, &o);
            match (&with, &t) {
                (Ok(a), Ok(b)) => assert!(a == b, "threaded indexed decode differs"),
                (a, b) => assert_eq!(a.is_ok(), b.is_ok(), "threaded indexed decode outcome"),
            }
        }
    }
    let _ = decode_and_index(
        data,
        &DecodeOptions {
            scale: Scale::Quarter,
            ..Default::default()
        },
    );
});
