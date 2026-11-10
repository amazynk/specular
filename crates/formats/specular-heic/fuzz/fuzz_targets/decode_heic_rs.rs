//! Fuzz the HEIC container + the forked heic-rs HEVC core (`hevc`), and
//! check that the core's serial and WPP-threaded paths agree byte for
//! byte, damaged streams included. Run from this directory with
//! `cargo +nightly fuzz run decode_heic_rs corpus/decode -- -max_total_time=900`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_heic::{DecodeOptions, HeicFile, HeicRs};

fuzz_target!(|data: &[u8]| {
    let Ok(f) = HeicFile::open(data.to_vec()) else {
        return;
    };
    let info = f.info();
    if info.width as u64 * info.height as u64 > 1 << 22 {
        return;
    }
    let serial = DecodeOptions {
        parallel: false,
        core: Some(&HeicRs),
        ..Default::default()
    };
    let threaded = DecodeOptions {
        parallel: true,
        ..serial
    };
    // Coded items: the root, else up to 4 grid tiles. `decode_item`
    // gives a threaded run the full in-picture budget.
    let root = f.graph();
    let ids: Vec<u32> = if root.inputs.is_empty() {
        vec![root.item.id]
    } else {
        root.inputs.iter().take(4).map(|n| n.item.id).collect()
    };
    for id in ids {
        let a = f.decode_item(id, &serial);
        let b = f.decode_item(id, &threaded);
        match (a, b) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.frame, b.frame, "serial/threaded frames differ");
                assert_eq!(a.concealed_ctus, b.concealed_ctus);
            }
            (Err(_), Err(_)) => {}
            (a, b) => panic!("serial {:?} vs threaded {:?}", a.err(), b.err()),
        }
    }
    if let Ok(img) = f.decode(&threaded) {
        let rgba = img.to_rgba8().expect("rgba of a decoded image");
        assert_eq!(rgba.len(), img.width() as usize * img.height() as usize * 4);
    }
});
