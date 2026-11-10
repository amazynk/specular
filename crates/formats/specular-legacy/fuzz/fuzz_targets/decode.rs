//! Fuzz every legacy format (PCX, SGI, Sun raster, XBM, XPM, WBMP, farbfeld, FLI/FLC): the
//! sniffing entry points, each format's decoder called directly (so a file of one format is
//! also thrown at the others), and FLI playback. Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_legacy::{farbfeld, fli, pcx, sgi, sun, wbmp, xbm, xpm, Image, Info, Result};

fn check(info: Result<Info>, img: impl FnOnce() -> Result<Image>) {
    let Ok(info) = info else { return };
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 22 {
        return;
    }
    if let Ok(img) = img() {
        assert_eq!(
            img.data.len(),
            img.width as usize * img.height as usize * img.format.channels()
        );
        assert_eq!(img.format, info.pixel_format);
    }
}

fuzz_target!(|data: &[u8]| {
    let _ = specular_legacy::detect(data);
    check(specular_legacy::info(data), || {
        specular_legacy::decode(data)
    });
    check(pcx::info(data), || pcx::decode(data));
    check(sgi::info(data), || sgi::decode(data));
    check(sun::info(data), || sun::decode(data));
    check(xbm::info(data), || xbm::decode(data));
    check(xpm::info(data), || xpm::decode(data));
    check(wbmp::info(data), || wbmp::decode(data));
    check(farbfeld::info(data), || farbfeld::decode(data));
    check(fli::info(data), || fli::decode_first(data));
    if let Ok(i) = fli::fli_info(data) {
        if i.width as u64 * i.height as u64 <= 1 << 20 {
            if let Ok(mut frames) = fli::FliFrames::new(data) {
                for _ in 0..2 {
                    for _ in 0..16 {
                        let Some(Ok(f)) = frames.next_frame() else {
                            break;
                        };
                        assert_eq!(f.image.data.len(), i.width as usize * i.height as usize * 4);
                    }
                    let _ = frames.rewind();
                }
            }
        }
    }
});
