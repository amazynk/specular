//! Edited thumbnails: the photo developed with its
//! edit at thumbnail size on the GPU, through the same graph as the preview and export.
//!
//! - [`read_raw`] reads a raw for it: its sensor data (the front end reduces it on the
//!   GPU); other images come decoded at a reduced scale ([`SOURCE_LONG`]).
//! - [`Thumbnailer::render`] develops it upright (the file's orientation, then the edit's
//!   turns and crop) into sRGB-coded RGBA rows, long side at most the size asked.
//!
//! The library keys the result by the file, the edit's output hash and
//! [`crate::graph::GRAPH_VERSION`] (`specular_thumbs::ThumbKey::edited`).

use std::path::Path;

use crate::gpu::Error;
use crate::gpu::pool::Budget;
use crate::model::Edit;
use crate::ops::correct::calib::{self, FrameInfo};
use crate::ops::output::OutputSpace;
use crate::preview::{self, Developer, Input, Orient};

/// The long side edited thumbnails are made at (the largest thumbnail class).
pub const LONG: u32 = 512;

/// The long side a raw is developed at for its thumbnail by default (Settings › Gallery ›
/// Sharper raw thumbnails), then reduced to [`LONG`]: twice the thumbnail, sharper and
/// closer to the export (develop-quality "Over the bar"), about 40 ms more per photo.
pub const RAW_LONG: u32 = 2 * LONG;

/// ... and with that setting off: at the thumbnail's own size.
pub const RAW_LONG_FAST: u32 = LONG;

/// The long side raws are developed at for thumbnails: [`RAW_LONG`] when `sharper`
/// (the default), else [`RAW_LONG_FAST`].
pub fn raw_long(sharper: bool) -> u32 {
    if sharper { RAW_LONG } else { RAW_LONG_FAST }
}

/// Non-raw sources are given at about this long side: twice the thumbnail, so neighbourhood
/// ops see detail like the preview does.
pub const SOURCE_LONG: u32 = 1024;

/// ... and at about this long side when the edit's warp resamples them ([`source_long`]):
/// the warp samples a picture twice the size it develops at, as the preview's does.
pub const SOURCE_LONG_WARPED: u32 = 2048;

/// The long side to give a non-raw at for `edit` ([`SOURCE_LONG`] or, when its warp
/// resamples the picture, [`SOURCE_LONG_WARPED`]).
pub fn source_long(edit: &Edit) -> u32 {
    if preview::rgb_warp_resamples(edit) {
        SOURCE_LONG_WARPED
    } else {
        SOURCE_LONG
    }
}

/// Raws whose visible long side is at least this are binned 2×2 for thumbnails (still
/// twice [`SOURCE_LONG`] after binning).
pub const BIN_FROM: u32 = 4 * LONG;

/// A file read for a thumbnail.
pub struct Source {
    pub input: Input,
    /// EXIF orientation (1..=8).
    pub orientation: u16,
    /// A raw's facts for matching flat and dark frames automatically.
    pub frame: Option<FrameInfo>,
    /// A raw's Lensfun lookup ([`crate::ops::geometry::lensfun::shot_of_raw`]), so the
    /// thumbnail has the lens correction Develop shows.
    pub lensfun: Option<std::sync::Arc<crate::ops::geometry::lensfun::Shot>>,
    /// The 35 mm equivalent focal length ([`crate::ops::geometry::persp::focal35_of_file`]),
    /// so perspective matches Develop's.
    pub focal35: Option<f64>,
}

/// Reads a raw's sensor data for [`Thumbnailer::render`] (the front end reduces it on the
/// GPU); `Ok(None)` when `path` is not a raw this build develops from its sensor data (the
/// caller decodes the image and gives it as [`Input::Rgb`] at about [`SOURCE_LONG`]).
pub fn read_raw(path: &Path) -> Result<Option<Source>, String> {
    if !may_be_raw(path) {
        return Ok(None);
    }
    let data = raw_bytes(path)?;
    let data: &[u8] = &data;
    Ok(match preview::decode_raw(data) {
        Ok(Input::Raw(s)) => {
            let orientation = s.meta.orientation;
            let lensfun = crate::ops::geometry::lensfun::shot_of_raw(&s, data);
            Some(Source {
                input: Input::Raw(s),
                orientation,
                frame: calib::frame_info(path, data),
                lensfun,
                focal35: crate::ops::geometry::persp::focal35_of_file(data),
            })
        }
        _ => None,
    })
}

/// Whether [`read_raw`] tries `path` as a raw (by its name; a raw `.mov` by its track):
/// false for images it never develops from sensor data, so their thumbnails do not
/// depend on [`Thumbnailer::raw_long`].
pub fn may_be_raw(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    !(NOT_RAW.contains(&ext.as_str()) || ext == "mov" && !is_raw_clip(path))
}

/// Whether `path` is a raw video clip Develop takes (its first frame): Canon Cinema RAW
/// Light (`.crm`), or a QuickTime movie (`.mov`) of ProRes RAW, told by its track's sample
/// description in the first or last megabyte.
pub fn is_raw_clip(path: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    const SPAN: u64 = 1 << 20;
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "crm" => true,
        "mov" | "qt" => {
            let Ok(mut f) = std::fs::File::open(path) else {
                return false;
            };
            let len = f.metadata().map_or(0, |m| m.len());
            let mut buf = Vec::new();
            if (&mut f).take(SPAN).read_to_end(&mut buf).is_err() {
                return false;
            }
            if len > SPAN {
                let at = (len - SPAN).max(SPAN);
                if f.seek(SeekFrom::Start(at)).is_err()
                    || (&mut f).take(SPAN).read_to_end(&mut buf).is_err()
                {
                    return false;
                }
            }
            specular_raw::detect(&buf) == Some(specular_raw::RawFormat::ProResRaw)
        }
        _ => false,
    }
}

/// A raw's bytes for Develop: read whole, or mapped for a raw video clip (clips run to
/// gigabytes; only the movie header and the first frame are touched).
pub enum RawBytes {
    Read(Vec<u8>),
    Mapped(specular_core::Source),
}

impl std::ops::Deref for RawBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            RawBytes::Read(v) => v,
            RawBytes::Mapped(s) => s.bytes(),
        }
    }
}

/// Reads (or, for a raw video clip, maps) the raw at `path`.
pub fn raw_bytes(path: &Path) -> Result<RawBytes, String> {
    if is_raw_clip(path) {
        return specular_core::Source::open(path)
            .map(RawBytes::Mapped)
            .map_err(|e| e.to_string());
    }
    // The local copy when the original is offline.
    specular_core::local_copy::read(path)
        .map(RawBytes::Read)
        .map_err(|e| e.to_string())
}

/// Extensions that are never raws.
const NOT_RAW: [&str; 16] = [
    "jpg", "jpeg", "png", "webp", "avif", "jxl", "heic", "heif", "tif", "tiff", "gif", "bmp",
    "exr", "hdr", "psd", "jp2",
];

/// Develops edited thumbnails on one device (pipelines stay compiled between photos).
pub struct Thumbnailer {
    dev: Developer,
    /// The long side a raw is developed at before it is reduced to the thumbnail's
    /// (`None`: the thumbnail's own; [`RAW_LONG`] by default); larger costs time, and runs
    /// neighbourhood ops and the tone curve on finer detail. Rounded to the nearest whole
    /// multiple of the thumbnail's: reduced by a factor like 1.5, every other thumbnail
    /// pixel straddles two developed ones and the picture blurs (768 px was worse than 512).
    pub raw_long: Option<u32>,
}

impl Thumbnailer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, budget: Budget) -> Thumbnailer {
        let mut dev = Developer::new(device, queue, budget);
        // One render per photo: it waits for the photo's own highlight statistics, so
        // clipped highlights get the colours the full render gives them.
        dev.wait_highlight_stats = true;
        Thumbnailer {
            dev,
            raw_long: Some(RAW_LONG),
        }
    }

    /// `edit` on `src`, upright and cropped, long side about `long`: (width, height,
    /// sRGB-coded RGBA). Waits for the GPU.
    pub fn render(
        &mut self,
        src: Source,
        edit: &Edit,
        long: u32,
    ) -> Result<(u32, u32, Vec<u8>), Error> {
        let d = &mut self.dev;
        // The flat and dark frames the edit asks for, as an export takes them. A
        // frame that can't be used leaves the thumbnail uncalibrated (Develop says why).
        let cal = match &src.input {
            Input::Raw(s) => calib::for_edit_at(edit, s, src.frame.as_ref())
                .ok()
                .flatten(),
            Input::Rgb { .. } => None,
        };
        // A large sensor is binned 2×2 per colour first (a quarter of the samples to
        // upload, gather statistics on and develop), X-Trans 3×3 into RGB (no demosaic);
        // not when frames calibrate it (they are the sensor's size).
        let mut input = src.input;
        // Raws at `raw_long`, other images at their given size up to [`SOURCE_LONG`]: a
        // whole multiple of `long`, so the reduction to it is one from the sensor (or the
        // image) as the export's is.
        let long = long.max(1);
        let multiple = match &input {
            Input::Raw(_) => {
                let r = self.raw_long.unwrap_or(long);
                ((r + long / 2) / long).max(1)
            }
            Input::Rgb { width, height, .. } => {
                ((*width).max(*height) / long).clamp(1, (SOURCE_LONG / long).max(1))
            }
        };
        let develop_long = long * multiple;
        let mut bin = 1.0;
        if let Input::Raw(s) = &input
            && cal.is_none()
            && s.crop.w.max(s.crop.h) >= BIN_FROM
            && let Some((h, k)) = s
                .half()
                .map(|h| (h, 2.0))
                .or_else(|| s.third().map(|t| (t, 3.0)))
        {
            input = Input::Raw(Box::new(h));
            bin = k;
        }
        d.set_input(input)?;
        d.set_lensfun(src.lensfun);
        d.set_focal35(src.focal35);
        d.sensor_bin = bin;
        d.set_calibration(cal.as_deref())?;
        d.base = Orient::from_exif(src.orientation);
        d.crop_on = true;
        d.size_multiple = multiple;
        d.prepare(edit, develop_long)?;
        let r = d.render(edit, None, &OutputSpace::rec709())?;
        let (w, h) = (r.roi.w, r.roi.h);
        // Larger than the thumbnail (a crop is developed larger, other images and
        // `raw_long` too): area-reduced in linear light before 8 bits.
        let out = if preview::fit(w, h, long) == (w, h) {
            d.read_rgba8(r, Orient::default())
        } else {
            let px = d.read(&r);
            d.recycle(r);
            px.map(|px| fit_linear(w, h, &px, long))
        };
        // The source goes: the next photo is another.
        d.set_input(Input::Rgb {
            width: 1,
            height: 1,
            rgba: vec![0.0; 4],
        })?;
        out
    }
}

/// sRGB-coded RGBA `w × h` area-reduced in linear light to fit a long side of `long`
/// (as it is when it fits). Exact area weights: a pixel straddling two output pixels
/// counts in both by the part in each (no aliasing at ratios like 1.5).
pub fn fit_rgba8(w: u32, h: u32, px: &[u8], long: u32) -> (u32, u32, Vec<u8>) {
    let (dw, dh) = preview::fit(w, h, long);
    if (dw, dh) == (w, h) {
        return (w, h, px.to_vec());
    }
    let lin: Vec<f32> = (0..256).map(|i| srgb_decode(i as f32 / 255.0)).collect();
    let out = area_reduce(w, h, dw, dh, |i| {
        let p = &px[i * 4..i * 4 + 4];
        [
            lin[usize::from(p[0])],
            lin[usize::from(p[1])],
            lin[usize::from(p[2])],
            f32::from(p[3]) / 255.0,
        ]
    });
    (dw, dh, out)
}

/// Linear RGBA `w × h` (a render read back) clipped to [0, 1] as a display shows it,
/// area-reduced in linear light to fit a long side of `long`, then sRGB-coded to 8 bits
/// (opaque, as [`Developer::read_rgba8`] gives it).
pub fn fit_linear(w: u32, h: u32, px: &[f32], long: u32) -> (u32, u32, Vec<u8>) {
    let (dw, dh) = preview::fit(w, h, long);
    let out = area_reduce(w, h, dw, dh, |i| {
        let p = &px[i * 4..i * 4 + 3];
        let c = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        [c(p[0]), c(p[1]), c(p[2]), 1.0]
    });
    (dw, dh, out)
}

fn srgb_decode(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_encode8(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

/// `w × h` pixels (`get(i)`: linear RGB and alpha in [0, 1]) area-reduced to `dw × dh`,
/// each output pixel the exact area mean of the source rectangle it covers, sRGB-coded.
fn area_reduce(
    w: u32,
    h: u32,
    dw: u32,
    dh: u32,
    get: impl Fn(usize) -> [f32; 4] + Sync,
) -> Vec<u8> {
    use rayon::prelude::*;
    // Per output column (row): the source columns it covers and each one's weight.
    let spans = |n: u32, dn: u32| -> Vec<(u32, Vec<f64>)> {
        let s = f64::from(n) / f64::from(dn);
        (0..dn)
            .map(|o| {
                let a = f64::from(o) * s;
                let b = (a + s).min(f64::from(n));
                let first = a.floor() as u32;
                let last = (b.ceil() as u32).clamp(first + 1, n);
                let ws = (first..last)
                    .map(|x| (f64::from(x) + 1.0).min(b) - f64::from(x).max(a))
                    .map(|v| v.max(0.0) / s)
                    .collect();
                (first, ws)
            })
            .collect()
    };
    let (xs, ys) = (spans(w, dw), spans(h, dh));
    let mut out = vec![0u8; dw as usize * dh as usize * 4];
    out.par_chunks_mut(dw as usize * 4)
        .zip(&ys)
        .for_each(|(row, (y0, wy))| {
            for ((x0, wx), o) in xs.iter().zip(row.chunks_exact_mut(4)) {
                let mut acc = [0f64; 4];
                for (j, &ky) in wy.iter().enumerate() {
                    let base = (*y0 as usize + j) * w as usize + *x0 as usize;
                    for (i, &kx) in wx.iter().enumerate() {
                        let k = kx * ky;
                        for (a, v) in acc.iter_mut().zip(get(base + i)) {
                            *a += k * f64::from(v);
                        }
                    }
                }
                for (d, a) in o[..3].iter_mut().zip(acc) {
                    *d = srgb_encode8(a as f32);
                }
                o[3] = (acc[3].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact area mean of `src` (linear RGB, w × h) over output pixel (ox, oy) of dw × dh.
    fn reference(
        src: &[f32],
        (w, h): (u32, u32),
        (dw, dh): (u32, u32),
        ox: u32,
        oy: u32,
    ) -> [f64; 3] {
        let (sx, sy) = (f64::from(w) / f64::from(dw), f64::from(h) / f64::from(dh));
        let (x0, x1) = (f64::from(ox) * sx, f64::from(ox + 1) * sx);
        let (y0, y1) = (f64::from(oy) * sy, f64::from(oy + 1) * sy);
        let mut acc = [0f64; 3];
        for y in 0..h {
            let wy = (f64::from(y + 1).min(y1) - f64::from(y).max(y0)).max(0.0);
            for x in 0..w {
                let wx = (f64::from(x + 1).min(x1) - f64::from(x).max(x0)).max(0.0);
                let i = (y * w + x) as usize * 4;
                for (c, a) in acc.iter_mut().enumerate() {
                    *a += wx * wy * f64::from(src[i + c]);
                }
            }
        }
        acc.map(|a| a / (sx * sy))
    }

    /// At 1.5× (and other ratios that are not whole) each output pixel is the exact area
    /// mean of what it covers, to within the 8-bit rounding.
    #[test]
    fn linear_reduction_is_the_exact_area_mean() {
        for (w, h, long) in [(768u32, 512u32, 512u32), (300, 200, 128), (97, 61, 40)] {
            let px: Vec<f32> = (0..w * h * 4)
                .map(|i| ((i.wrapping_mul(2_654_435_761) >> 8) % 1000) as f32 / 999.0)
                .collect();
            let (dw, dh, out) = fit_linear(w, h, &px, long);
            assert_eq!((dw, dh), preview::fit(w, h, long));
            for oy in (0..dh).step_by(7) {
                for ox in (0..dw).step_by(5) {
                    let r = reference(&px, (w, h), (dw, dh), ox, oy);
                    let o = (oy * dw + ox) as usize * 4;
                    for c in 0..3 {
                        let want = srgb_encode8(r[c] as f32);
                        assert!(
                            out[o + c].abs_diff(want) <= 1,
                            "{w}x{h} → {dw}x{dh} at ({ox}, {oy}): {} vs {want}",
                            out[o + c]
                        );
                    }
                    assert_eq!(out[o + 3], 255);
                }
            }
        }
    }

    /// Linear RGBA `src` area-reduced to `to` with exact weights (the front end's and
    /// `fit_linear`'s reduction).
    fn reduced(src: &[f32], from: (u32, u32), to: (u32, u32)) -> Vec<f32> {
        let mut out = Vec::with_capacity((to.0 * to.1 * 4) as usize);
        for oy in 0..to.1 {
            for ox in 0..to.0 {
                let r = reference(src, from, to, ox, oy);
                out.extend(r.map(|v| v as f32));
                out.push(1.0);
            }
        }
        out
    }

    /// Why 768 px raw thumbnails were worse than 512: reducing a picture already
    /// reduced from the sensor equals one reduction from the sensor only by a whole factor.
    /// At 1.5× every other thumbnail pixel takes part of a developed pixel that also covers
    /// sensor pixels outside it (a wider, blurrier filter than the export's reduction); at 2×
    /// the developed pixels tile each thumbnail pixel exactly.
    #[test]
    fn two_reductions_equal_one_only_by_a_whole_factor() {
        let (w, h) = (225u32, 150u32);
        // Stripes off the pixel grid and a hard edge.
        let px: Vec<f32> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let v = if ((x + 0.37) / 6.15).floor() % 2.0 == 0.0 {
                    0.05
                } else {
                    0.6
                };
                let v = if y < 20.0 { 0.9 } else { v };
                [v, v * 0.5 + 0.1, 1.0 - v, 1.0]
            })
            .collect();
        let thumb = preview::fit(w, h, 64);
        let once = reduced(&px, (w, h), thumb);
        let worst = |dev: (u32, u32)| {
            let d = reduced(&px, (w, h), dev);
            let twice = reduced(&d, dev, thumb);
            once.iter()
                .zip(&twice)
                .map(|(a, b)| (a - b).abs())
                .fold(0f32, f32::max)
        };
        // 1.5×: off by up to a fifth of the stripes' contrast.
        assert!(worst((96, 64)) > 0.05, "{}", worst((96, 64)));
        // 2× and 3×, the develop size `fit_multiple` gives: the same to rounding.
        for m in [2, 3] {
            let dev = preview::fit_multiple(w, h, 64 * m, m);
            assert_eq!(dev, (thumb.0 * m, thumb.1 * m));
            assert!(worst(dev) < 1e-5, "{m}×: {}", worst(dev));
        }
    }

    #[test]
    fn develop_sizes_are_whole_multiples_of_the_thumbnail() {
        // 1800 × 1200 → 512 × 341: 2× is 1024 × 682, not fit's 1024 × 683.
        assert_eq!(preview::fit(1800, 1200, 1024), (1024, 683));
        assert_eq!(preview::fit_multiple(1800, 1200, 1024, 2), (1024, 682));
        assert_eq!(preview::fit_multiple(1800, 1200, 512, 1), (512, 341));
        // Not enough pixels for the multiple: as fit gives it.
        assert_eq!(preview::fit_multiple(600, 400, 1024, 2), (600, 400));
        assert_eq!(preview::fit_multiple(700, 400, 1024, 2), (700, 400));
    }

    /// The 8-bit path weighs pixels as the linear one: the same thumbnail from the same
    /// (8-bit) picture.
    #[test]
    fn eight_bit_reduction_equals_the_linear_one() {
        let (w, h) = (768u32, 300u32);
        let px8: Vec<u8> = (0..w * h * 4)
            .map(|i| {
                if i % 4 == 3 {
                    255
                } else {
                    (i.wrapping_mul(2_654_435_761) >> 13) as u8
                }
            })
            .collect();
        let px: Vec<f32> = px8
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let c = f32::from(v) / 255.0;
                if i % 4 == 3 { c } else { srgb_decode(c) }
            })
            .collect();
        let (dw, dh, out) = fit_linear(w, h, &px, 512);
        let (dw8, dh8, out8) = fit_rgba8(w, h, &px8, 512);
        assert_eq!((dw, dh), (dw8, dh8));
        let worst = out.iter().zip(&out8).map(|(a, b)| a.abs_diff(*b)).max();
        assert!(worst <= Some(1), "{worst:?}");
    }
}
