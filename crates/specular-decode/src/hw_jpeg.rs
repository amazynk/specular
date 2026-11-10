//! Whole JPEGs on our decoder or the hardware decoder (`specular-hwdec`): the path selector's
//! [`Task::JpegDecode`] (DESIGN.md §12.3.1). Every whole full-size decode is timed for it.
//!
//! Measured on a Radeon 860M (VA-API, 16 threads): alone, the hardware decoder is about four
//! times slower than our software decoder on all threads (50 MP 4:2:0: 211 ms vs 54 ms), mostly
//! from reading the surface back out of uncached memory. But it runs beside the CPU: while one
//! full-size image decodes in software, a second full-size decode in software slows the first
//! from 51 to 91 ms; on the hardware decoder the first stays at 57 ms, and it costs a fraction
//! of the CPU time. So on that machine the selector keeps the image the user waits for in
//! software and gives background prefetch to the hardware (lowest CPU time that keeps up);
//! a discrete GPU's faster decoder can win outright. The device takes one decode at a time.
//!
//! The first hardware decode (an exploring background request) is also decoded in software
//! and compared ([`specular_hwdec::check_quality`]); below the PSNR floor the path is
//! blocklisted. A driver error blocklists it too; the image is then decoded in software.

use crate::DecodeRequest;
use crate::Result;
use crate::jpeg::to_raster;
use specular_core::paths::{
    Choice, Engine, Fallback, PathSelector, Request, ScaleFallback, Scaled, Task, note_checked,
    note_fallback, note_scale_fallback, note_scaled, selector,
};
use specular_core::{CancelToken, Raster};
use specular_jpeg as sj;
use std::sync::atomic::{AtomicBool, Ordering};

/// A hardware decode in flight.
static HARDWARE: AtomicBool = AtomicBool::new(false);

pub(crate) struct HardwareSlot;

impl HardwareSlot {
    /// The device for one decode, unless a decode is already on it.
    pub(crate) fn take() -> Option<HardwareSlot> {
        HARDWARE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
            .then_some(HardwareSlot)
    }
}

impl Drop for HardwareSlot {
    fn drop(&mut self) {
        HARDWARE.store(false, Ordering::Release);
    }
}

/// Whether the request is a whole baseline 8-bit image at full size, the work both paths do.
fn whole(ji: &sj::JpegInfo, req: &DecodeRequest) -> bool {
    req.scale <= 1
        && req.region.is_none()
        && matches!(
            ji.coding,
            sj::Coding::Baseline | sj::Coding::ExtendedHuffman
        )
        && ji.precision == 8
}

/// The path for `req` in `sel`: `None` when only our decoder can do it.
fn choose(sel: &PathSelector, ji: &sj::JpegInfo, req: &DecodeRequest) -> Option<Choice> {
    whole(ji, req).then(|| {
        let px = u64::from(ji.width) * u64::from(ji.height);
        sel.choose(Task::JpegDecode, &Request::new(px, 0).verifying())
    })
}

/// Decodes `req` on the chosen path; `software` is our decoder (the reference and fallback).
pub(crate) fn decode(
    data: &[u8],
    ji: &sj::JpegInfo,
    req: &DecodeRequest,
    cancel: &CancelToken,
    software: impl FnOnce() -> Result<Raster>,
) -> Result<Raster> {
    decode_in(selector(), data, ji, req, cancel, software)
}

/// [`decode`] with the choice and timings in `sel` (the process's selector outside tests,
/// which must not share it: other tests decode, calibrate and save it concurrently).
fn decode_in(
    sel: &'static PathSelector,
    data: &[u8],
    ji: &sj::JpegInfo,
    req: &DecodeRequest,
    cancel: &CancelToken,
    software: impl FnOnce() -> Result<Raster>,
) -> Result<Raster> {
    let Some(choice) = choose(sel, ji, req) else {
        // Scaled or region decodes: not timed, but they load the CPU.
        let _busy = sel.busy();
        note_level(ji, req);
        return software();
    };
    let cpu = if choice.engine == Engine::HwDecoder {
        match hardware(sel, data, ji, &choice, cancel) {
            Some(r) => return Ok(r),
            None => choice.reference(),
        }
    } else {
        choice
    };
    let run = sel.start(&cpu);
    let r = software()?;
    run.finish();
    Ok(r)
}

/// Notes how a reduced-size whole level is decoded, for the open report: on our decoder (the
/// hardware decoder only takes whole full-size images), at which scale, and whether the coding
/// has to be decoded at full size first.
fn note_level(ji: &sj::JpegInfo, req: &DecodeRequest) {
    if req.region.is_some() || req.scale <= 1 {
        return;
    }
    let px = u64::from(ji.width) * u64::from(ji.height);
    note_scaled(
        Task::JpegDecode,
        px,
        Scaled {
            requested: req.scale.min(255) as u8,
            dct: crate::jpeg::scale_of(req.scale).factor() as u8,
            fallback: None,
        },
    );
    if ji.coding == sj::Coding::Lossless {
        note_scale_fallback(ScaleFallback::FullSize);
    }
}

/// One decode on the hardware; `None`: decode in software instead.
fn hardware(
    sel: &PathSelector,
    data: &[u8],
    ji: &sj::JpegInfo,
    choice: &Choice,
    cancel: &CancelToken,
) -> Option<Raster> {
    if !specular_hwdec::enabled() {
        note_fallback(Fallback::Unavailable);
        return None;
    }
    if !specular_hwdec::jpeg_fits(ji.width, ji.height) {
        note_fallback(Fallback::Unsupported);
        return None;
    }
    let Some(_slot) = HardwareSlot::take() else {
        note_fallback(Fallback::Busy);
        return None;
    };
    let run = sel.start(choice);
    let hw = match specular_hwdec::decode_jpeg(data) {
        Ok(r) => r,
        Err(e @ specular_hwdec::Error::Hardware(_)) => {
            note_fallback(Fallback::Failed);
            run.fail(&e.to_string());
            return None;
        }
        Err(e) => {
            note_fallback(match e {
                specular_hwdec::Error::Unavailable => Fallback::Unavailable,
                _ => Fallback::Unsupported,
            });
            specular_log::debug!(target: specular_log::DECODE, "jpeg: hardware decode not used: {e}");
            return None;
        }
    };
    let ms = run.elapsed_us() / 1e3;
    run.finish();
    specular_log::debug!(target: specular_log::DECODE,
        "jpeg: hardware decode {}×{} in {ms:.1} ms ({})", hw.width, hw.height, choice.reason.key());
    if choice.verify {
        note_checked();
        let opts = sj::DecodeOptions {
            cancel: Some(cancel.flag()),
            ..Default::default()
        };
        let sw = to_raster(sj::decode(data, &opts).ok()?, 1);
        specular_hwdec::check_quality(&hw, &sw);
        // The software result either way: it is the reference.
        return Some(sw);
    }
    Some(hw)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This test's own selector: the process's is shared with tests that decode (in
    /// flight), calibrate (may trust the hardware decoder) and take its dirty count.
    static SEL: PathSelector = PathSelector::new();

    #[test]
    fn only_whole_full_size_baseline_images_have_a_choice() {
        let mut jpeg = Vec::new();
        jpeg_encoder::Encoder::new(&mut jpeg, 90)
            .encode(&[128u8; 16 * 16 * 3], 16, 16, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        let ji = sj::read_info(&jpeg).unwrap();
        assert_eq!(ji.coding, sj::Coding::Baseline);
        // Nothing registered the hardware decoder: our decoder, as the only path.
        let c = choose(&SEL, &ji, &DecodeRequest::full(1)).unwrap();
        assert_eq!(c.engine, Engine::Cpu);
        assert!(choose(&SEL, &ji, &DecodeRequest::full(2)).is_none());
        // A reduced level notes our decoder, its scale and (for lossless files) the
        // full-size decode it needs.
        let _ = specular_core::paths::take_note();
        let r = decode_in(
            &SEL,
            &jpeg,
            &ji,
            &DecodeRequest::full(2),
            &CancelToken::new(),
            || {
                Ok(to_raster(
                    sj::decode(
                        &jpeg,
                        &sj::DecodeOptions {
                            scale: sj::Scale::Half,
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                    2,
                ))
            },
        )
        .unwrap();
        assert_eq!(r.width, 8);
        let n = specular_core::paths::take_note().unwrap();
        assert_eq!(
            (n.choice.task, n.choice.engine, n.choice.reason),
            (
                Task::JpegDecode,
                Engine::Cpu,
                specular_core::paths::Reason::Scaled
            )
        );
        assert_eq!(
            n.scaled,
            Some(Scaled {
                requested: 2,
                dct: 2,
                fallback: None
            })
        );
        let lossless = sj::JpegInfo {
            coding: sj::Coding::Lossless,
            ..ji.clone()
        };
        note_level(&lossless, &DecodeRequest::full(8));
        let n = specular_core::paths::take_note().unwrap();
        assert_eq!(
            n.scaled,
            Some(Scaled {
                requested: 8,
                dct: 1,
                fallback: Some(ScaleFallback::FullSize)
            })
        );
        // Regions (tiles) and full-size decodes leave no reduced-size note.
        let mut tile = DecodeRequest::full(2);
        tile.region = Some(crate::Rect {
            x: 0,
            y: 0,
            w: 4,
            h: 4,
        });
        note_level(&ji, &tile);
        note_level(&ji, &DecodeRequest::full(1));
        assert_eq!(specular_core::paths::take_note(), None);
        let progressive = sj::JpegInfo {
            coding: sj::Coding::Progressive,
            ..ji.clone()
        };
        assert!(choose(&SEL, &progressive, &DecodeRequest::full(1)).is_none());
        // Every whole decode is timed for the selector.
        let before = SEL.take_dirty();
        let r = decode_in(
            &SEL,
            &jpeg,
            &ji,
            &DecodeRequest::full(1),
            &CancelToken::new(),
            || {
                Ok(to_raster(
                    sj::decode(&jpeg, &Default::default()).unwrap(),
                    1,
                ))
            },
        )
        .unwrap();
        assert_eq!(r.width, 16);
        let _ = before;
        assert!(SEL.take_dirty() >= 1);
        assert_eq!(SEL.inflight(), 0);
    }
}
