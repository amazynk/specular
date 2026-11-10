//! Calibration of the decoders' dual paths on built-in samples (DESIGN.md §12.3.1): run while
//! the machine is idle, on a normal-priority background thread (so the timings match an image
//! the user waits for), about a second in all. Each path is timed end to end and its output
//! compared with the CPU reference before the selector trusts it.

use specular_core::paths::{Engine, Task, Trust, png_variant, selector};
use specular_png::{ColorType, DecodeOptions, Output, Parallelism, Wavefront};

/// A synthetic RGBA image: `flat` like a screenshot (long runs, compresses ≥ 32:1), otherwise
/// a smooth gradient with grain like a photo (compresses about 2–3:1).
fn sample(w: u32, h: u32, flat: bool) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    let mut seed = 0x9e37_79b9u32;
    for y in 0..h {
        for x in 0..w {
            if flat {
                let tile = ((x / 64) ^ (y / 48)) & 3;
                let c = [240u8, 32, 128, 200][tile as usize];
                let edge = u8::from(x % 64 == 0 || y % 48 == 0) * 60;
                px.extend_from_slice(&[c, c.wrapping_add(edge), 255 - c, 255]);
            } else {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let n = (seed & 15) as u8;
                px.extend_from_slice(&[
                    ((x * 255 / w) as u8).saturating_add(n),
                    ((y * 255 / h) as u8).saturating_add(n),
                    (((x + y) * 127 / (w + h)) as u8).saturating_add(n / 2),
                    255,
                ]);
            }
        }
    }
    px
}

/// Times wavefront and row-by-row PNG unfiltering on two built-in images (a screenshot-like and
/// a photo-like one, 4 MP each) and checks that both give the same pixels; trusts the
/// wavefront if they do, blocklists it otherwise. Returns false if cancelled by `stop`.
pub fn png(stop: &dyn Fn() -> bool) -> bool {
    let sel = selector();
    sel.set_available(Task::PngUnfilter, Engine::CpuParallel);
    let (w, h) = (2048u32, 2048u32);
    let px = u64::from(w) * u64::from(h);
    let mut same = true;
    for flat in [true, false] {
        let Ok(png) = specular_png::encode(w, h, ColorType::Rgba, &sample(w, h, flat)) else {
            return true;
        };
        let variant = png_variant((u64::from(w) * 4 + 1) * u64::from(h), png.len() as u64);
        let decode = |wavefront| {
            let o = DecodeOptions {
                output: Output::Rgba8,
                parallelism: Parallelism::Threads,
                wavefront,
                ..Default::default()
            };
            specular_png::decode(&png, &o).ok().map(|i| i.planes)
        };
        // Warm the allocator and the thread pool once, then time each path twice, alternating.
        let reference = decode(Wavefront::Off);
        for _ in 0..2 {
            for (path, wf) in [(0, Wavefront::Off), (1, Wavefront::On)] {
                if stop() {
                    return false;
                }
                let run = sel.start(&sel.calibration(Task::PngUnfilter, path, px, variant));
                let out = decode(wf);
                run.finish();
                same &= out.is_some()
                    && out
                        .as_ref()
                        .map(|p| p.iter().map(|p| &p.data).collect::<Vec<_>>())
                        == reference
                            .as_ref()
                            .map(|p| p.iter().map(|p| &p.data).collect());
            }
        }
    }
    sel.verified(Task::PngUnfilter, Engine::CpuParallel, same);
    true
}

/// The HEIC calibration sample: a 2048×1024 photo-like picture (`heic-calibration.py`) coded
/// by libheif/x265 at quality 50 as a 4×2 grid of 512×512 tiles, 8-bit 4:2:0, with a 256×128
/// thumbnail (237 kB, about 0.9 bit per pixel, like a phone photo):
/// `heif-enc -q 50 --cut-tiles 512 -t 256 -o heic-calibration.heic sample.png`.
pub(crate) const HEIC_SAMPLE: &[u8] = include_bytes!("../data/heic-calibration.heic");

/// Times the software HEVC core and, when the hardware decoder is allowed and usable, VA-API on the
/// built-in sample (whole decodes, alternating, twice each after a warm-up), and compares the
/// hardware's pictures with software's: identical trusts the hardware path, different
/// blocklists it. Returns false if cancelled by `stop`.
pub fn heic(stop: &dyn Fn() -> bool) -> bool {
    use specular_heic::{DEFAULT_CORE, DecodeOptions, HeicFile, HevcCore};
    use specular_hwdec::heic::{Policy, VaapiCore};
    let sel = selector();
    let Ok(file) = HeicFile::open(HEIC_SAMPLE) else {
        return true;
    };
    let px = u64::from(file.info().width) * u64::from(file.info().height);
    let tiles = file.tile_count();
    let hardware = (sel.trust(Task::HeicDecode, 1) != Trust::Absent
        && sel.trust(Task::HeicDecode, 1) != Trust::Blocked
        && specular_hwdec::enabled()
        && specular_hwdec::heic::usable())
    .then(|| VaapiCore::new().with_policy(Policy::Always));
    let decode = |core: &dyn HevcCore| {
        file.decode(&DecodeOptions {
            parallel: true,
            cancel: None,
            core: Some(core),
        })
        .ok()
        .map(|i| i.frame.planes)
    };
    let reference = decode(&DEFAULT_CORE);
    let mut same = None;
    if let Some(hw) = &hardware {
        let before = hw.stats().0;
        let out = decode(hw);
        if hw.stats().0 - before == tiles {
            same = Some(out == reference);
        }
    }
    for _ in 0..2 {
        if stop() {
            return false;
        }
        let run = sel.start(&sel.calibration(Task::HeicDecode, 0, px, 0));
        std::hint::black_box(decode(&DEFAULT_CORE));
        run.finish();
        if let (Some(hw), Some(true)) = (&hardware, same) {
            if stop() {
                return false;
            }
            let before = hw.stats().0;
            let run = sel.start(&sel.calibration(Task::HeicDecode, 1, px, 0));
            std::hint::black_box(decode(hw));
            if hw.stats().0 - before == tiles {
                run.finish();
            }
        }
    }
    if let Some(same) = same {
        specular_hwdec::heic::record_check(same);
    }
    true
}

/// The JPEG calibration sample: a 2368×1776 photo-like picture (4.2 MP, the smallest size
/// bucket of phone photos; `heic-calibration.py`) coded by libjpeg-turbo as baseline 8-bit
/// 4:2:0 at quality 85 without restart markers (507 kB, about 1 bit per pixel):
/// `cjpeg -quality 85 -sample 2x2 -optimize -outfile jpeg-calibration.jpg sample.ppm`.
pub(crate) const JPEG_SAMPLE: &[u8] = include_bytes!("../data/jpeg-calibration.jpg");

/// Times our decoder and, when the hardware decoder is allowed, usable and idle, VA-API on the
/// built-in sample (whole decodes, alternating, twice each after a warm-up), and compares the
/// hardware's pictures with ours ([`specular_hwdec::check_quality`]: IDCTs differ, so by PSNR
/// rather than bit for bit): close enough trusts the hardware path, otherwise it is
/// blocklisted. Returns false if cancelled by `stop`.
pub fn jpeg(stop: &dyn Fn() -> bool) -> bool {
    use specular_jpeg as sj;
    let sel = selector();
    let Ok(info) = sj::read_info(JPEG_SAMPLE) else {
        return true;
    };
    let px = u64::from(info.width) * u64::from(info.height);
    let software = || {
        sj::decode(JPEG_SAMPLE, &sj::DecodeOptions::default())
            .ok()
            .map(|d| crate::jpeg::to_raster(d, 1))
    };
    // Held throughout, so no background decode shares the device with the timed runs.
    let slot = (sel.trust(Task::JpegDecode, 1) != Trust::Absent
        && sel.trust(Task::JpegDecode, 1) != Trust::Blocked
        && specular_hwdec::enabled()
        && specular_hwdec::capabilities().is_some_and(|c| c.jpeg.is_some())
        && specular_hwdec::jpeg_fits(info.width, info.height))
    .then(crate::hw_jpeg::HardwareSlot::take)
    .flatten();
    let reference = software();
    let mut same = false;
    if slot.is_some()
        && let Some(r) = &reference
    {
        match specular_hwdec::decode_jpeg(JPEG_SAMPLE) {
            Ok(hw) => same = specular_hwdec::check_quality(&hw, r),
            Err(e @ specular_hwdec::Error::Hardware(_)) => {
                sel.block(Task::JpegDecode, Engine::HwDecoder, &e.to_string());
            }
            Err(_) => {}
        }
    }
    for _ in 0..2 {
        if stop() {
            return false;
        }
        let run = sel.start(&sel.calibration(Task::JpegDecode, 0, px, 0));
        std::hint::black_box(software());
        run.finish();
        if same {
            if stop() {
                return false;
            }
            let run = sel.start(&sel.calibration(Task::JpegDecode, 1, px, 0));
            match specular_hwdec::decode_jpeg(JPEG_SAMPLE) {
                Ok(hw) => {
                    run.finish();
                    std::hint::black_box(hw);
                }
                Err(e @ specular_hwdec::Error::Hardware(_)) => {
                    run.fail(&e.to_string());
                    same = false;
                }
                Err(_) => same = false,
            }
        }
    }
    drop(slot);
    true
}

/// Whether a decoder calibration has no timing in the selector's table yet: a table calibrated
/// before that calibration existed (the fingerprint does not change between dev builds) or on a
/// run that skipped it. The app then runs [`run`] again once.
pub fn missing() -> bool {
    let sel = selector();
    let heic = sel.calibration(Task::HeicDecode, 0, HEIC_PIXELS, 0);
    let jpeg = sel.calibration(Task::JpegDecode, 0, JPEG_PIXELS, 0);
    sel.samples(Task::HeicDecode, 0, heic.key) == 0
        || sel.samples(Task::JpegDecode, 0, jpeg.key) == 0
}

/// The calibration samples' sizes ([`HEIC_SAMPLE`], [`JPEG_SAMPLE`]).
const HEIC_PIXELS: u64 = 2048 * 1024;
const JPEG_PIXELS: u64 = 2368 * 1776;

/// Everything the decoders calibrate (see [`png`], [`heic`], [`jpeg`]).
pub fn run(stop: &dyn Fn() -> bool) -> bool {
    png(stop) && heic(stop) && jpeg(stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hardware calibrations share the global selector and the VA-API device; the app runs
    /// them one after another on one thread, so the tests do too.
    static HARDWARE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn samples_cover_both_ends_and_the_wavefront_is_verified() {
        for (flat, lo, hi) in [(true, 32, u64::MAX), (false, 0, 8)] {
            let (w, h) = (512, 512);
            let png = specular_png::encode(w, h, ColorType::Rgba, &sample(w, h, flat)).unwrap();
            let ratio = u64::from(w * 4 * h) / png.len() as u64;
            assert!((lo..hi).contains(&ratio), "flat {flat}: {ratio}:1");
        }
        assert!(png(&|| false));
        // `--nocapture`: this machine's measurements.
        eprintln!("{}", selector().to_table());
        assert_eq!(selector().trust(Task::PngUnfilter, 1), Trust::Trusted);
        assert!(!png(&|| true));
    }

    #[test]
    fn heic_calibration_times_software_and_checks_hardware() {
        let _one = HARDWARE.lock().unwrap_or_else(|e| e.into_inner());
        // As the app does with hardware decoding on "auto" and its probe thread.
        specular_hwdec::set_enabled(true);
        let hw = specular_hwdec::probe().is_some_and(|c| c.hevc.is_some());
        assert!(heic(&|| false));
        eprintln!("{}", selector().to_table());
        let key = selector()
            .calibration(Task::HeicDecode, 0, HEIC_PIXELS, 0)
            .key;
        assert!(selector().samples(Task::HeicDecode, 0, key) >= 2);
        if hw && specular_hwdec::heic::usable() {
            // Identical pictures on this machine's driver.
            assert_eq!(selector().trust(Task::HeicDecode, 1), Trust::Trusted);
            let key = selector()
                .calibration(Task::HeicDecode, 1, HEIC_PIXELS, 0)
                .key;
            assert!(selector().samples(Task::HeicDecode, 1, key) >= 2);
        }
        assert!(!heic(&|| true));
    }

    #[test]
    fn jpeg_calibration_times_software_and_checks_hardware() {
        let _one = HARDWARE.lock().unwrap_or_else(|e| e.into_inner());
        let ji = specular_jpeg::read_info(JPEG_SAMPLE).unwrap();
        assert_eq!(ji.coding, specular_jpeg::Coding::Baseline);
        assert_eq!(u64::from(ji.width) * u64::from(ji.height), JPEG_PIXELS);
        assert_eq!(specular_core::paths::bucket(JPEG_PIXELS), 3);
        // As the app does with hardware decoding on "auto" and its probe thread.
        specular_hwdec::set_enabled(true);
        let hw = specular_hwdec::probe().is_some_and(|c| c.jpeg.is_some());
        assert!(jpeg(&|| false));
        eprintln!("{}", selector().to_table());
        let key = selector()
            .calibration(Task::JpegDecode, 0, JPEG_PIXELS, 0)
            .key;
        assert!(selector().samples(Task::JpegDecode, 0, key) >= 2);
        if hw && specular_hwdec::enabled() {
            // Within the PSNR floor of our decoder on this machine's driver.
            assert_eq!(selector().trust(Task::JpegDecode, 1), Trust::Trusted);
            let key = selector()
                .calibration(Task::JpegDecode, 1, JPEG_PIXELS, 0)
                .key;
            assert!(selector().samples(Task::JpegDecode, 1, key) >= 2);
        }
        assert!(!jpeg(&|| true));
        // With HEIC's calibration too (again, if its test ran first), nothing is missing.
        assert!(heic(&|| false));
        assert!(!missing());
    }
}
