//! The full-resolution render of one image for export.
//!
//! The source is made at full size (the raw front end in f32 storage when the device can
//! hold it, else f16), then the edit's graph with the output transform into the export
//! space runs in bands of rows in f32 ([`Request`] with checkpoints off; the engine tiles a
//! band when it must). Each band is read back, resized in linear light when the size asks
//! for it, encoded with the space's curve (or kept linear for float files) and written into
//! the output; the result is turned upright at the end.

use std::time::Instant;

use rayon::prelude::*;

use super::preset::{Depth, Preset};
use super::quantise::{GpuThresholds, Quantiser, Thresholds};
use super::resample::Resampler;
use super::{Space, Transfer};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Error, Request, Source};
use crate::graph::tile::Tiling;
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::model::params::{ModuleParams, RawFrontEnd};
use crate::model::{Edit, ModuleKind, Params};
use crate::ops::detail::ImageDetail;
use crate::ops::geometry::warp::Warper;
use crate::ops::geometry::{Context, Plan};
use crate::ops::raw::{FrontEnd, Options};
use crate::preview::{Input, Orient};

/// Bytes of one band's f32 output (and of its read-back copy).
const BAND_BYTES: u64 = 192 << 20;
/// Largest f32 source kept on the GPU (above it the source is f16).
const F32_SOURCE_MAX: u64 = 3 << 30;

/// Interleaved RGB samples, upright, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub enum Samples {
    /// Encoded with the space's curve.
    Eight(Vec<u8>),
    Sixteen(Vec<u16>),
    /// Linear light in the space's primaries.
    Float(Vec<f32>),
}

/// Where the time of one export went (wall clock).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExportStats {
    /// Sensor upload and the raw front end (or the upload of other images).
    pub source_ms: f64,
    /// Graph bands, read-back, resize and encoding to samples.
    pub render_ms: f64,
    pub bands: usize,
    /// Storage of the developed source.
    pub source_f32: bool,
    /// Denoise before the warp ([`crate::prewarp`]; part of `source_ms`), when it ran: to its
    /// last band submitted (the GPU finishes it while the warp is prepared).
    pub prewarp_ms: f64,
    /// The most GPU memory in use while it ran (bytes): the source, its denoised copy and
    /// the band render's buffers (the engine's free and kept caches aside).
    pub prewarp_peak: u64,
    /// The samples were coded on the GPU (8- and 16-bit, no resize).
    pub gpu_codes: bool,
    /// The resize's horizontal half ran on the GPU.
    pub gpu_resize: bool,
}

/// One rendered image.
#[derive(Clone, Debug, PartialEq)]
pub struct Exported {
    pub width: u32,
    pub height: u32,
    pub samples: Samples,
    pub stats: ExportStats,
    /// The gain map towards the HDR rendition (gain-map exports, [`super::hdr`]).
    pub gain_map: Option<specular_convert::gain_map::GainMapImage>,
    /// Coverage 0..1 per pixel (upright, `width × height`), when the source has
    /// transparency ([`super::hdr::render`] adds it).
    pub alpha: Option<Vec<f32>>,
}

/// One rendition of an export: the edit (the same as the others' but for the view
/// transform), the preset and the peak it is clipped at.
pub struct Rendition<'a> {
    pub edit: &'a Edit,
    pub preset: &'a Preset,
    pub peak: f32,
}

/// One rendition being rendered.
struct Lane {
    graph: Graph,
    resampler: Option<Resampler>,
    out: Out,
    curve: Curve,
    /// 8-bit or 16-bit codes made on the GPU (no resize): the band is read
    /// back as packed codes.
    codes: Option<GpuThresholds>,
    /// A resize's horizontal half on the GPU: the band is read back resized across.
    hresize: Option<super::hresize::HResize>,
    /// Output size (stored orientation), and upright.
    stored: (u32, u32),
    upright: (u32, u32),
}

/// The export renderer on one device (reused across a batch: pipelines stay compiled).
pub struct Exporter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    engine: Engine,
    front: FrontEnd,
    warper: Warper,
    max_buffer: u64,
    max_binding: u64,
    /// Two read-back buffers, used in turn.
    staging: Vec<Option<wgpu::Buffer>>,
    /// Integer codes on the GPU (made at the first 8- or 16-bit export).
    quantiser: Option<Quantiser>,
    /// Each curve's thresholds on the GPU, by transfer and bits (the 16-bit ones
    /// take a moment to find, so they are kept for the batch).
    thresholds: Vec<(Transfer, u32, GpuThresholds)>,
    /// The last render ended with both read-back buffers unmapped (else they are made anew).
    staging_ok: bool,
    /// Flat-field and dark-frame calibration for the next raw rendered (the caller
    /// sets it per picture, [`crate::ops::correct::calib::for_edit`]).
    pub calibration: Option<std::sync::Arc<crate::ops::correct::calib::Calibration>>,
    /// The aperture and pixel pitch of the next picture rendered (capture sharpening's
    /// automatic radius, [`crate::ops::detail::capture::lens_of_file`]; taken once).
    pub lens: Option<(Option<f32>, Option<f32>)>,
    /// The Lensfun lookup of the next raw rendered
    /// ([`crate::ops::geometry::lensfun::shot_of_raw`]; taken once).
    pub lensfun: Option<std::sync::Arc<crate::ops::geometry::lensfun::Shot>>,
    /// The picture's 35 mm equivalent focal length, for perspective
    /// ([`crate::ops::geometry::persp::focal35_of_file`]).
    pub focal35: Option<f64>,
    /// The identity of the next picture rendered ([`crate::mask::ai::photo_id`]): keys
    /// denoise's AI result ([`crate::ops::detail::ai`]).
    pub photo: Option<[u8; 32]>,
    /// The next raw's noise model when fitted ahead ([`crate::ops::detail::RawNoise::of_sensor`]
    /// of the same sensor; the batch's reader fits it off the GPU thread); taken
    /// once, else fitted here.
    pub raw_noise: Option<crate::ops::detail::RawNoise>,
    /// 8- and 16-bit samples coded on the GPU (the same codes, `false` codes
    /// on the CPU).
    pub gpu_codes: bool,
    /// A resized export's horizontal half on the GPU (`false`: all of it on the CPU, the
    /// reference).
    pub gpu_resize: bool,
    /// Profile the GPU work (the bench): each render's milliseconds per dispatch
    /// label are added to [`Exporter::profiled`] (the front end's passes as `raw <pass>`).
    pub profile: bool,
    /// GPU milliseconds per label of the renders since this was last emptied.
    pub profiled: Vec<(String, f64)>,
}

/// A band on its way back to the host.
struct Pending {
    staging: wgpu::Buffer,
    bytes: u64,
    /// The bytes that hold data (the rest pads to 4).
    len: u64,
    /// Bits of the packed codes (8 or 16; 0: f32 RGBA).
    codes: u32,
    /// f32 RGB rows already resized across ([`super::hresize`]).
    resized: bool,
    /// Its first row.
    y: u32,
    index: wgpu::SubmissionIndex,
    mapped: std::sync::mpsc::Receiver<bool>,
}

/// Waits for `p` and hands its bytes (f32 RGBA rows, or packed codes) to `f`.
fn finish(device: &wgpu::Device, p: Pending, f: &mut dyn FnMut(u32, &[u8])) -> Result<(), Error> {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(p.index),
            timeout: None,
        })
        .map_err(|e| Error::Gpu(e.to_string()))?;
    if p.mapped.recv_timeout(std::time::Duration::from_secs(60)) != Ok(true) {
        return Err(Error::Gpu("reading the export back failed".into()));
    }
    {
        let view = p
            .staging
            .slice(..p.bytes)
            .get_mapped_range()
            .map_err(|e| Error::Gpu(format!("{e:?}")))?;
        f(p.y, &view[..p.len as usize]);
    }
    p.staging.unmap();
    Ok(())
}

/// Values stored for a linear value, tabulated over `u = l^¼` (smooth near black for
/// every curve), linearly interpolated.
pub(super) struct Curve {
    table: Vec<f32>,
}

const CURVE_STEPS: usize = 1 << 14;

impl Curve {
    pub(super) fn new(t: Transfer) -> Curve {
        Curve {
            table: (0..=CURVE_STEPS)
                .map(|i| {
                    let u = i as f64 / CURVE_STEPS as f64;
                    t.encode(u * u * u * u) as f32
                })
                .collect(),
        }
    }

    /// The 8-bit code of `l`.
    #[inline]
    pub(super) fn code8(&self, l: f32) -> u8 {
        (self.at(l) * 255.0 + 0.5) as u8
    }

    /// The 16-bit code of `l`.
    #[inline]
    pub(super) fn code16(&self, l: f32) -> u16 {
        (self.at(l) * 65535.0 + 0.5) as u16
    }

    #[inline]
    pub(super) fn at(&self, l: f32) -> f32 {
        let u = l.clamp(0.0, 1.0).sqrt().sqrt() * CURVE_STEPS as f32;
        let i = (u as usize).min(CURVE_STEPS - 1);
        let f = u - i as f32;
        self.table[i] + (self.table[i + 1] - self.table[i]) * f
    }
}

#[cfg(test)]
pub(crate) fn curve_for_tests(t: Transfer) -> impl Fn(f32) -> f32 {
    let c = Curve::new(t);
    move |l| c.at(l)
}

/// The output being filled (stored orientation, 3 samples per pixel).
enum Out {
    Eight(Vec<u8>),
    Sixteen(Vec<u16>),
    Float(Vec<f32>),
}

impl Out {
    fn new(depth: Depth, n: usize) -> Out {
        match depth {
            Depth::Eight => Out::Eight(vec![0; n]),
            Depth::Sixteen => Out::Sixteen(vec![0; n]),
            Depth::Half | Depth::Float => Out::Float(vec![0.0; n]),
        }
    }

    /// Writes packed codes (3 per pixel; 8-bit, or 16-bit little-endian) of rows from `y`
    /// on.
    fn put_codes(&mut self, y: usize, w: usize, codes: &[u8]) {
        let at = y * w * 3;
        match self {
            Out::Eight(v) => {
                let n = codes.len();
                v[at..at + n]
                    .par_chunks_mut(1 << 20)
                    .zip(codes.par_chunks(1 << 20))
                    .for_each(|(o, s)| o.copy_from_slice(s));
            }
            Out::Sixteen(v) => {
                let n = codes.len() / 2;
                v[at..at + n]
                    .par_chunks_mut(1 << 19)
                    .zip(codes.par_chunks(1 << 20))
                    .for_each(|(o, s)| {
                        for (o, s) in o.iter_mut().zip(s.chunks_exact(2)) {
                            *o = u16::from_le_bytes([s[0], s[1]]);
                        }
                    });
            }
            Out::Float(_) => unreachable!("codes are made for integer outputs only"),
        }
    }

    /// Writes rows from `y` on: `src` has `cin` values per pixel (RGB first), `w` pixels a row.
    fn put(&mut self, y: usize, w: usize, src: &[f32], cin: usize, curve: &Curve) {
        let rows = src.len() / (w * cin);
        let at = y * w * 3;
        let n = rows * w * 3;
        match self {
            Out::Eight(v) => v[at..at + n]
                .par_chunks_mut(w * 3)
                .zip(src.par_chunks(w * cin))
                .for_each(|(o, s)| {
                    for (o, s) in o.chunks_exact_mut(3).zip(s.chunks_exact(cin)) {
                        for c in 0..3 {
                            o[c] = curve.code8(s[c]);
                        }
                    }
                }),
            Out::Sixteen(v) => v[at..at + n]
                .par_chunks_mut(w * 3)
                .zip(src.par_chunks(w * cin))
                .for_each(|(o, s)| {
                    for (o, s) in o.chunks_exact_mut(3).zip(s.chunks_exact(cin)) {
                        for c in 0..3 {
                            o[c] = curve.code16(s[c]);
                        }
                    }
                }),
            Out::Float(v) => v[at..at + n]
                .par_chunks_mut(w * 3)
                .zip(src.par_chunks(w * cin))
                .for_each(|(o, s)| {
                    for (o, s) in o.chunks_exact_mut(3).zip(s.chunks_exact(cin)) {
                        for c in 0..3 {
                            o[c] = s[c].max(0.0);
                        }
                    }
                }),
        }
    }

    /// Turned by `o` (from a `w × h` stored image; unturned, the samples themselves).
    fn finish(self, w: usize, h: usize, o: Orient) -> Samples {
        if o == Orient::default() {
            return match self {
                Out::Eight(v) => Samples::Eight(v),
                Out::Sixteen(v) => Samples::Sixteen(v),
                Out::Float(v) => Samples::Float(v),
            };
        }
        match self {
            Out::Eight(v) => Samples::Eight(turn(&v, w, h, o)),
            Out::Sixteen(v) => Samples::Sixteen(turn(&v, w, h, o)),
            Out::Float(v) => Samples::Float(turn(&v, w, h, o)),
        }
    }
}

/// `src` (`w × h`, 3 per pixel) turned by `o`.
pub fn turn<T: Copy + Default + Send + Sync>(src: &[T], w: usize, h: usize, o: Orient) -> Vec<T> {
    if o == Orient::default() {
        return src.to_vec();
    }
    let (ow, _) = o.size(w as u32, h as u32);
    let ow = ow as usize;
    let mut out = vec![T::default(); src.len()];
    // In 64 × 64 tiles: a quarter turn reads the source down its columns, and a row at a
    // time touched a new cache line (and page) for every pixel.
    const TILE: usize = 64;
    out.par_chunks_mut(ow * 3 * TILE)
        .enumerate()
        .for_each(|(by, band)| {
            let rows = band.len() / (ow * 3);
            for x0 in (0..ow).step_by(TILE) {
                let x1 = (x0 + TILE).min(ow);
                for dy in 0..rows {
                    let y = (by * TILE + dy) as u32;
                    let row = &mut band[dy * ow * 3..(dy + 1) * ow * 3];
                    for x in x0..x1 {
                        let (sx, sy) = o.source_of(x as u32, y, w as u32, h as u32);
                        let s = (sy as usize * w + sx as usize) * 3;
                        row[x * 3..x * 3 + 3].copy_from_slice(&src[s..s + 3]);
                    }
                }
            }
        });
    out
}

/// The developed size of `input` (stored orientation) for `edit`, before any resize.
pub fn source_size(input: &Input) -> (u32, u32) {
    match input {
        Input::Raw(s) => (s.crop.w, s.crop.h),
        Input::Rgb { width, height, .. } => (*width, *height),
    }
}

/// The upright output size of a `w × h` stored source turned by `o` under `preset`.
pub fn output_size(w: u32, h: u32, o: Orient, preset: &Preset) -> (u32, u32) {
    let (dw, dh) = o.size(w, h);
    specular_convert::output_size(dw, dh, preset.size, preset.upscale)
}

impl Exporter {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, budget: Budget) -> Exporter {
        let limits = device.limits();
        Exporter {
            device: device.clone(),
            queue: queue.clone(),
            engine: Engine::new(device, queue, budget),
            front: FrontEnd::new(device, queue),
            warper: Warper::new(device, queue),
            max_buffer: limits.max_buffer_size,
            max_binding: limits
                .max_storage_buffer_binding_size
                .min(limits.max_buffer_size),
            staging: vec![None, None],
            quantiser: None,
            thresholds: Vec::new(),
            gpu_codes: true,
            gpu_resize: true,
            profile: false,
            profiled: Vec::new(),
            staging_ok: true,
            calibration: None,
            lens: None,
            lensfun: None,
            focal35: None,
            photo: None,
            raw_noise: None,
        }
    }

    /// The GPU thresholds of `curve` (the curve of `transfer`) for `bits`-bit codes, made
    /// once per transfer and depth.
    fn gpu_thresholds(&mut self, transfer: Transfer, bits: u32, curve: &Curve) -> GpuThresholds {
        if let Some((.., t)) = self
            .thresholds
            .iter()
            .find(|(tr, b, _)| *tr == transfer && *b == bits)
        {
            return t.clone();
        }
        let t = if bits == 8 {
            Thresholds::new(8, |l| u16::from(curve.code8(l)))
        } else {
            Thresholds::new(16, |l| curve.code16(l))
        };
        let g = self
            .quantiser
            .get_or_insert_with(|| Quantiser::new(&self.device, &self.queue))
            .thresholds(&t);
        self.thresholds.push((transfer, bits, g.clone()));
        g
    }

    /// A profiled render's GPU milliseconds into [`Exporter::profiled`].
    fn take_profile(&mut self, r: &crate::gpu::Rendered) {
        if let Some(v) = &r.stats.gpu_ms {
            self.profiled.extend(v.iter().cloned());
        }
    }

    /// Copies `r` (rows from `y`) into read-back buffer `slot` and asks for it mapped: its
    /// f32 pixels, or with `codes` their 8- or 16-bit codes (packed RGB).
    fn read_back(
        &mut self,
        r: &crate::gpu::Rendered,
        y: u32,
        slot: usize,
        codes: Option<&GpuThresholds>,
        hresize: Option<&mut super::hresize::HResize>,
    ) -> Pending {
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("develop export read"),
            });
        let resized = hresize.is_some() && r.precision == Precision::F32;
        let (from, bytes, len) = match (codes, hresize) {
            (_, Some(h)) if r.precision == Precision::F32 => {
                let (b, n) = h.record(&mut enc, r.buffer(), r.roi.h, slot);
                (b, n.div_ceil(4) * 4, Some((n, 0)))
            }
            (Some(t), _) if r.precision == Precision::F32 => {
                let q = self
                    .quantiser
                    .get_or_insert_with(|| Quantiser::new(&self.device, &self.queue));
                let (b, n) = q.record(&mut enc, r.buffer(), r.roi.pixels(), t, slot);
                (
                    b,
                    n,
                    Some((r.roi.pixels() * 3 * u64::from(t.bits / 8), t.bits)),
                )
            }
            _ => (r.buffer().clone(), r.bytes().div_ceil(4) * 4, None),
        };
        let codes = len.map_or(0, |l| l.1);
        let len = len.map_or(r.bytes(), |l| l.0);
        if self.staging.len() <= slot {
            self.staging.resize(slot + 1, None);
        }
        let staging = match &self.staging[slot] {
            Some(b) if b.size() >= bytes => b.clone(),
            _ => {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("develop export read"),
                    size: bytes,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.staging[slot] = Some(b.clone());
                b
            }
        };
        enc.copy_buffer_to_buffer(&from, 0, &staging, 0, bytes);
        let index = self.queue.submit([enc.finish()]);
        let (tx, mapped) = std::sync::mpsc::channel();
        staging
            .slice(..bytes)
            .map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res.is_ok());
            });
        Pending {
            staging,
            bytes,
            len,
            codes,
            resized,
            y,
            index,
            mapped,
        }
    }

    /// Makes the full-size source of `input`, the warp's view of it (shown with `base`) and
    /// the image's facts for the detail ops.
    fn source(
        &mut self,
        input: Input,
        edit: &Edit,
        base: Orient,
    ) -> Result<(Source, bool, Context, ImageDetail), Error> {
        self.engine.clear_cache();
        let denoise = edit
            .modules
            .iter()
            .any(|m| m.kind == ModuleKind::DENOISE && m.enabled);
        let mut detail = ImageDetail {
            photo: self.photo,
            ..ImageDetail::default()
        };
        if let Some((f, pitch)) = self.lens.take() {
            detail.capture_sigma = Some(crate::ops::detail::capture::auto_sigma(f, pitch));
        }
        match input {
            Input::Rgb {
                width,
                height,
                rgba,
            } => {
                if denoise {
                    detail.noise = crate::ops::detail::noise::estimate_rgb(width, height, &rgba);
                }
                let mut ctx = Context::image(width, height, base);
                ctx.focal35 = self.focal35;
                // RGB chromatic aberration in the warp.
                ctx.rgb_ca = crate::ops::correct::chromatic_aberration(edit).and_then(|p| {
                    if p.auto {
                        crate::ops::correct::ca::estimate_rgb(width, height, &rgba)
                            .map(|k| k.map(|k| k * 100.0))
                    } else {
                        Some([p.red, p.blue])
                    }
                });
                Ok((self.engine.upload(width, height, &rgba)?, true, ctx, detail))
            }
            Input::Raw(mut s) => {
                // The input profile file stands in for the file's colour (warps included);
                // the matrix when it cannot be used.
                let front = raw_front_end(edit);
                let _ = crate::ops::raw::profile_file::apply(&mut s.meta, &front);
                let mut ctx = Context::raw(&s, base);
                ctx.lensfun = self.lensfun.take();
                ctx.focal35 = self.focal35;
                // Bayer raws correct chromatic aberration on the CFA; other layouts take the
                // manual scales in the warp.
                if !matches!(s.meta.layout, specular_raw::sensor::Layout::Bayer(_)) {
                    ctx.rgb_ca = crate::ops::correct::chromatic_aberration(edit)
                        .filter(|p| !p.auto)
                        .map(|p| [p.red, p.blue]);
                }
                let p = front;
                let (w, h) = (s.crop.w, s.crop.h);
                let ahead = self.raw_noise.take();
                let raw_noise = denoise
                    .then(|| ahead.unwrap_or_else(|| crate::ops::detail::RawNoise::of_sensor(&s)));
                let mut gpu = self.front.upload(*s)?;
                let mut o = Options::from_params(&p, 1.0);
                o.defer_warps = true;
                o.profile_gpu = self.profile;
                // Hot pixels, raw CA and calibration from their modules.
                let c = crate::ops::correct::Corrections::of_edit(edit);
                let cal = self.calibration.take();
                self.front.set_calibration(&mut gpu, cal.as_deref())?;
                o.corrections = crate::ops::correct::Corrections {
                    highlights: o.corrections.highlights,
                    highlight_threshold: o.corrections.highlight_threshold,
                    calibrate: c.calibrate && cal.is_some(),
                    ..c
                };
                let f32_bytes = u64::from(w) * u64::from(h) * Precision::F32.bytes_per_pixel();
                o.precision = if f32_bytes <= self.max_buffer.min(F32_SOURCE_MAX) {
                    Precision::F32
                } else {
                    Precision::F16
                };
                let out = match self.front.run(&gpu, &o) {
                    Err(Error::TooLarge { .. }) if o.precision == Precision::F32 => {
                        o.precision = Precision::F16;
                        self.front.run(&gpu, &o)?
                    }
                    r => r?,
                };
                self.profiled.extend(out.stats.gpu_passes.iter().cloned());
                if let Some(raw) = &raw_noise {
                    detail.noise = Some(crate::ops::raw::noise_model(gpu.sensor(), raw, &o, 1.0));
                }
                drop(gpu);
                self.front.release_scratch();
                let f32 = out.precision == Precision::F32;
                let src = self.engine.source_from_buffer(
                    out.buffer,
                    out.width,
                    out.height,
                    out.precision,
                )?;
                Ok((src, f32, ctx, detail))
            }
        }
    }

    /// `src` through the edit's geometry warp (orientation included), at the source's own
    /// resolution; `src` itself when the warp moves nothing. Returns the orientation still
    /// to apply.
    fn warp(
        &mut self,
        edit: &Edit,
        ctx: &Context,
        src: Source,
        orient: Orient,
    ) -> Result<(Source, Orient), Error> {
        let plan = Plan::new(edit, ctx, true);
        if plan.is_identity() {
            return Ok((src, orient));
        }
        // Quarter turns and flips alone: turned on the CPU after rendering (no second
        // full-size buffer).
        if plan.orientation_only() {
            return Ok((src, plan.orient));
        }
        let (w, h) = plan.output_size(f64::from(src.width) / ctx.frame[2]);
        let buf = self.warper.run(
            &plan,
            src.buffer(),
            src.width,
            src.height,
            src.precision,
            w,
            h,
            src.precision,
        )?;
        let out = self.engine.source_from_buffer(buf, w, h, src.precision)?;
        drop(src);
        Ok((out, Orient::default()))
    }

    /// `before` (denoise and what precedes it) rendered on the unwarped `src` over the region
    /// `plan`'s warp reads ([`crate::prewarp`]), in bands of rows, into a new source of
    /// `src`'s size; `src` is dropped. Checks `progress` (at 0) between bands for Stop. With
    /// the most memory held meanwhile ([`ExportStats::prewarp_peak`]; 0 when nothing ran).
    fn denoise_before_warp(
        &mut self,
        before: &Edit,
        ctx: &Context,
        plan: &Plan,
        src: Source,
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<(Source, u64), Error> {
        let mut masks = crate::mask::MaskCache::default();
        let mut env = crate::mask::MaskEnv {
            geometry: crate::mask::Geometry::Frame {
                w: ctx.frame[2],
                h: ctx.frame[3],
            },
            image: (src.width, src.height),
            scale: 1.0,
            cache: &mut masks,
            sensor_bin: 1.0,
            overlay: None,
        };
        let graph = Graph::build_masked(before, OpRegistry::builtin(), None, None, Some(&mut env))
            .map_err(|e| {
                Error::Unsupported(match e {
                    crate::graph::GraphError::ParamsTooLarge(n) => n,
                })
            })?;
        if graph.groups.is_empty() {
            return Ok((src, 0));
        }
        let need = crate::prewarp::source_box(plan, src.width, src.height);
        let prec = src.precision;
        let bpp = prec.bytes_per_pixel();
        let bytes = (u64::from(src.width) * u64::from(src.height) * bpp).max(16);
        if bytes > self.max_buffer {
            return Err(Error::TooLarge { needs: bytes });
        }
        let dst = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop export denoised source"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cap = BAND_BYTES.min(self.max_binding / 2);
        let row = u64::from(need.w) * bpp;
        let band = (cap / row.max(1)).clamp(8, u64::from(need.h)) as u32;
        // Held throughout: the source and the denoised copy; per band, what the engine holds.
        let (_, _, live0) = self.engine.memory();
        let held = src.buffer().size() + bytes;
        let mut peak = held;
        let mut y = need.y;
        while y < need.bottom() {
            let rows = band.min(need.bottom() - y);
            let roi = Roi::new(need.x, y, need.w, rows);
            let req = Request {
                roi,
                scale: 1.0,
                precision: prec,
                tiling: Tiling::Auto,
                checkpoints: false,
                keep_result: false,
                profile: self.profile,
                checkpoint_every: false,
            };
            let r = self.engine.render(&graph, &src, &req)?;
            self.take_profile(&r);
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("develop export denoised source"),
                });
            crate::prewarp::copy_into(&mut enc, r.buffer(), roi, &dst, src.width, bpp);
            self.queue.submit([enc.finish()]);
            let (_, _, live) = self.engine.memory();
            peak = peak.max(held + live.saturating_sub(live0));
            self.engine.recycle(r);
            y += rows;
            if !progress(0.0) {
                return Err(Error::BadRequest("cancelled"));
            }
        }
        let out = self
            .engine
            .source_from_buffer(dst, src.width, src.height, prec)?;
        drop(src);
        self.engine.clear_cache();
        // Not waited for: the warp's dispatches follow on the queue, and the host
        // prepares them (and the edit's graph) while the GPU finishes denoising.
        Ok((out, peak))
    }

    /// Runs denoise's AI network over the whole frame (`src`, the source before the warp)
    /// when `edit` mixes in a result that is not done yet and the app set a runner
    /// ([`crate::ops::detail::ai::runner`]). Reports into the first half of `progress`.
    /// True when it ran.
    fn ai_denoise(
        &mut self,
        edit: &Edit,
        src: &Source,
        noise: Option<crate::ops::detail::NoiseModel>,
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<bool, Error> {
        use crate::ops::detail::ai;
        let Some((key, model)) = ai::pending(edit) else {
            return Ok(false);
        };
        let Some(runner) = ai::runner().lock().ok().and_then(|r| r.clone()) else {
            return Ok(false);
        };
        // The modules before denoise (the raw stage is in the source already).
        let reg = crate::model::Registry::builtin();
        let cut = reg.rank(ModuleKind::DENOISE);
        let mut before = edit.clone();
        for m in &mut before.modules {
            let warp = reg
                .get(m.kind)
                .is_some_and(|s| s.stage == crate::model::Stage::Warp);
            if reg.rank(m.kind) >= cut || warp {
                m.enabled = false;
            }
        }
        let graph = Graph::build(&before, OpRegistry::builtin(), None)
            .map_err(|_| Error::Unsupported("ai denoise input"))?;
        let req = Request {
            precision: Precision::F32,
            checkpoints: false,
            ..Request::new(Roi::full(src.width, src.height))
        };
        let r = self.engine.render(&graph, src, &req)?;
        let px = self.engine.read(&r);
        self.engine.recycle(r);
        let px = px?;
        let mut half = |f: f32| progress(0.5 * f);
        match runner.run(
            &key,
            &model,
            (src.width, src.height),
            &px,
            noise,
            None,
            &mut half,
        ) {
            Ok(true) => Ok(true),
            Ok(false) => Err(Error::BadRequest("cancelled")),
            // The classic result stands in (the export still happens; the runner logs why).
            Err(_) => Ok(true),
        }
    }

    /// Renders `edit` on `input` as `preset` asks, turned by `orient`. `progress` gets the
    /// fraction done after each band and stops the render when it returns false
    /// (`Err(Error::BadRequest("cancelled"))`).
    pub fn render(
        &mut self,
        input: Input,
        edit: &Edit,
        orient: Orient,
        preset: &Preset,
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<Exported, Error> {
        self.render_at(input, edit, orient, preset, 1.0, progress)
    }

    /// [`Exporter::render`] with the output clipped at `peak` (units of SDR white) instead
    /// of 1: HDR renditions ([`super::hdr`]), in float samples.
    pub fn render_at(
        &mut self,
        input: Input,
        edit: &Edit,
        orient: Orient,
        preset: &Preset,
        peak: f32,
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<Exported, Error> {
        let r = Rendition { edit, preset, peak };
        let mut v = self.render_renditions(input, orient, &[r], progress)?;
        Ok(v.remove(0))
    }

    /// Renders several renditions of one source at once: the source, its warp and every
    /// group before the view transform are made once (the group before the view transform
    /// is kept per band and the later renditions start from it), then each rendition's view
    /// transform and output. The renditions' edits must differ in the view transform only;
    /// the first one's source, warp and masks serve all.
    pub fn render_renditions(
        &mut self,
        input: Input,
        orient: Orient,
        list: &[Rendition<'_>],
        progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<Vec<Exported>, Error> {
        let Some(first) = list.first() else {
            return Ok(Vec::new());
        };
        let t0 = Instant::now();
        let (src, source_f32, ctx, detail) = self.source(input, first.edit, orient)?;
        let with: Vec<Option<Edit>> = list
            .iter()
            .map(|r| crate::ops::detail::with_image(r.edit, &detail))
            .collect();
        let edits: Vec<&Edit> = list
            .iter()
            .zip(&with)
            .map(|(r, w)| w.as_ref().unwrap_or(r.edit))
            .collect();
        // Denoise's AI result over the whole frame, run first when not done yet.
        let ai_ran = self.ai_denoise(edits[0], &src, detail.noise, progress)?;
        let mut progress = |f: f32| progress(if ai_ran { 0.5 + 0.5 * f } else { f });
        let progress: &mut dyn FnMut(f32) -> bool = &mut progress;
        // Denoise before a warp that resamples (`crate::prewarp`), as the preview does.
        let plan = Plan::new(edits[0], &ctx, true);
        let split: Vec<Option<(Edit, Edit)>> = if crate::prewarp::wanted(&plan) {
            edits.iter().map(|e| crate::prewarp::split(e)).collect()
        } else {
            Vec::new()
        };
        let tp = Instant::now();
        let (src, prewarp_peak) = match split.first() {
            Some(Some((before, _))) => {
                self.denoise_before_warp(before, &ctx, &plan, src, progress)?
            }
            _ => (src, 0),
        };
        let prewarp_ms = if prewarp_peak > 0 {
            tp.elapsed().as_secs_f64() * 1e3
        } else {
            0.0
        };
        let edits: Vec<&Edit> = edits
            .iter()
            .enumerate()
            .map(|(i, e)| match split.get(i) {
                Some(Some((_, after))) => after,
                _ => *e,
            })
            .collect();
        let (src, orient) = self.warp(edits[0], &ctx, src, orient)?;
        let source_ms = t0.elapsed().as_secs_f64() * 1e3;
        let t1 = Instant::now();
        let (sw, sh) = (src.width, src.height);
        // Masks: shapes in source coordinates, through the warp when it ran.
        let plan = (crate::mask::has_blends(edits[0])
            || crate::ops::retouch::needs_geometry(edits[0])
            || crate::ops::detail::ai::needs_geometry(edits[0]))
        .then(|| Plan::new(edits[0], &ctx, true));
        let geometry = match &plan {
            Some(p) if !(p.is_identity() || p.orientation_only()) => crate::mask::Geometry::Warp(p),
            _ => crate::mask::Geometry::Frame {
                w: ctx.frame[2],
                h: ctx.frame[3],
            },
        };
        let mut masks = crate::mask::MaskCache::default();
        let shared = list.len() > 1;
        let mut lanes = Vec::with_capacity(list.len());
        for (r, edit) in list.iter().zip(&edits) {
            let preset = r.preset.clone().normalised();
            let space: Space = preset.space;
            let output = crate::ops::output::OutputSpace {
                peak: r.peak,
                ..space.output()
            };
            let mut env = crate::mask::MaskEnv {
                geometry,
                image: (sw, sh),
                scale: 1.0,
                cache: &mut masks,
                sensor_bin: 1.0,
                overlay: None,
            };
            // Shared renders split before the view transform, so the group before it is a
            // checkpoint the later renditions start from.
            let focus = edit
                .modules
                .iter()
                .find(|m| m.kind == ModuleKind::VIEW_TRANSFORM && m.enabled)
                .map(|m| m.id())
                .filter(|_| shared);
            let graph = Graph::build_masked(
                edit,
                OpRegistry::builtin(),
                focus,
                Some(&output),
                Some(&mut env),
            )
            .map_err(|e| {
                Error::Unsupported(match e {
                    crate::graph::GraphError::ParamsTooLarge(n) => n,
                })
            })?;
            let (dw, dh) = output_size(sw, sh, orient, &preset);
            // The size in stored orientation.
            let (tw, th) = if orient.transpose { (dh, dw) } else { (dw, dh) };
            let resize = (tw, th) != (sw, sh);
            let curve = Curve::new(if preset.depth.float() {
                Transfer::Linear
            } else {
                space.transfer()
            });
            // 8- and 16-bit codes on the GPU, the CPU's (`quantise`).
            let bits = match preset.depth {
                Depth::Eight => 8,
                Depth::Sixteen => 16,
                Depth::Half | Depth::Float => 0,
            };
            let codes = (bits > 0 && !resize && self.gpu_codes)
                .then(|| self.gpu_thresholds(space.transfer(), bits, &curve));
            let resampler = resize.then(|| Resampler::new(sw, sh, tw, th));
            let hresize = resampler
                .as_ref()
                .filter(|_| self.gpu_resize)
                .map(|r| super::hresize::HResize::new(&self.device, &self.queue, sw, r.columns()));
            lanes.push(Lane {
                graph,
                hresize,
                resampler,
                out: Out::new(preset.depth, tw as usize * th as usize * 3),
                curve,
                codes,
                stored: (tw, th),
                upright: (dw, dh),
            });
        }
        let n = lanes.len();
        let cap = BAND_BYTES.min(self.max_binding / 2);
        let row_bytes = u64::from(sw) * Precision::F32.bytes_per_pixel();
        let band = (cap / row_bytes.max(1)).clamp(8, u64::from(sh)) as u32;
        let mut bands = 0;
        let mut y = 0;
        // Band k is read back and encoded on the CPU while the GPU renders band k + 1.
        let device = self.device.clone();
        if !self.staging_ok {
            self.staging.iter_mut().for_each(|b| *b = None);
            if let Some(q) = &mut self.quantiser {
                q.release();
            }
        }
        self.staging_ok = false;
        let mut pending: Vec<(usize, Pending)> = Vec::new();
        let take = |lanes: &mut [Lane], (k, p): (usize, Pending)| {
            let l = &mut lanes[k];
            let tw = l.stored.0;
            let (out, curve, resampler) = (&mut l.out, &l.curve, &mut l.resampler);
            let codes = p.codes;
            let resized = p.resized;
            finish(&device, p, &mut |y0: u32, bytes: &[u8]| {
                if codes > 0 {
                    return out.put_codes(y0 as usize, sw as usize, bytes);
                }
                let owned;
                let px = match bytemuck::try_cast_slice::<u8, f32>(bytes) {
                    Ok(px) => px,
                    Err(_) => {
                        owned = bytemuck::pod_collect_to_vec::<u8, f32>(bytes);
                        &owned[..]
                    }
                };
                match resampler {
                    Some(rs) => {
                        let (first, rgb) = if resized {
                            rs.push_resized(px)
                        } else {
                            rs.push(px)
                        };
                        out.put(first, tw as usize, &rgb, 3, curve);
                    }
                    None => out.put(y0 as usize, sw as usize, px, 4, curve),
                }
            })
        };
        while y < sh {
            let rows = band.min(sh - y);
            let req = Request {
                roi: Roi::new(0, y, sw, rows),
                scale: 1.0,
                precision: Precision::F32,
                tiling: Tiling::Auto,
                checkpoints: shared,
                keep_result: false,
                profile: self.profile,
                checkpoint_every: false,
            };
            let mut next = Vec::with_capacity(n);
            for (k, lane) in lanes.iter_mut().enumerate() {
                let r = self.engine.render(&lane.graph, &src, &req)?;
                self.take_profile(&r);
                let slot = (bands % 2) * n + k;
                let p = self.read_back(&r, y, slot, lane.codes.as_ref(), lane.hresize.as_mut());
                next.push((k, p));
                self.engine.recycle(r);
            }
            // This band's shared groups are no longer needed.
            if shared {
                self.engine.drop_checkpoints();
            }
            for p in std::mem::replace(&mut pending, next) {
                take(&mut lanes, p)?;
            }
            bands += 1;
            y += rows;
            if !progress(y as f32 / sh as f32) {
                return Err(Error::BadRequest("cancelled"));
            }
        }
        for p in std::mem::take(&mut pending) {
            take(&mut lanes, p)?;
        }
        self.staging_ok = true;
        drop(src);
        self.engine.clear_cache();
        let render_ms = t1.elapsed().as_secs_f64() * 1e3;
        let out = lanes
            .into_iter()
            .map(|l| {
                debug_assert!(l.resampler.as_ref().is_none_or(Resampler::done));
                let (tw, th) = l.stored;
                let gpu_codes = l.codes.is_some();
                let gpu_resize = l.hresize.is_some();
                Exported {
                    width: l.upright.0,
                    height: l.upright.1,
                    samples: l.out.finish(tw as usize, th as usize, orient),
                    stats: ExportStats {
                        source_ms,
                        render_ms,
                        bands,
                        source_f32,
                        prewarp_ms,
                        prewarp_peak,
                        gpu_codes,
                        gpu_resize,
                    },
                    gain_map: None,
                    alpha: None,
                }
            })
            .collect();
        let _ = self.device.poll(wgpu::PollType::Poll);
        Ok(out)
    }
}

/// The edit's raw front-end params (defaults when it has none enabled).
fn raw_front_end(edit: &Edit) -> RawFrontEnd {
    match edit
        .modules
        .iter()
        .find(|m| m.kind == ModuleKind::RAW_FRONT_END && m.enabled)
        .map(|m| &m.params)
    {
        Some(Params::RawFrontEnd(p)) => p.clone(),
        Some(other) => RawFrontEnd::from_fields(&other.to_fields()),
        None => RawFrontEnd::default(),
    }
}
