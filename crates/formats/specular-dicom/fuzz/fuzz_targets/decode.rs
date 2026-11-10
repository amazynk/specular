//! Fuzz the DICOM parser and pixel pipeline: Part 10 / preamble-less / deflated parsing,
//! sequences, fragments and offset tables, every native layout, RLE, the JPEG and JPEG 2000
//! hand-offs, the modality / VOI mapping and the icon. Frames are capped so runs stay cheap.
//! Run from this directory with `cargo +nightly fuzz run decode seeds`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_dicom::{Dicom, sniff};

fuzz_target!(|data: &[u8]| {
    let _ = sniff(data);
    let Ok(d) = Dicom::parse(data) else { return };
    let _ = d.summary(data);
    if let Some(Ok(icon)) = d.icon(data) {
        assert_eq!(icon.bytes() > 0, icon.width * icon.height > 0);
    }
    if d.pixel.as_ref().is_some_and(|p| p.pixels() > 1 << 20) {
        return;
    }
    let n = d.frame_count();
    for k in [0, n.saturating_sub(1)].into_iter().take(n.min(2) as usize) {
        let Ok(m) = d.modality_frame(data, k, None) else {
            continue;
        };
        let _ = m.value_range();
        let _ = m.value_at(0, 0);
        if let Ok(img) = m.render_default() {
            assert_eq!((img.width, img.height), (m.width(), m.height()));
        }
    }
});
