//! The raw front end on the GPU (stage [A] of the pipeline): the module kind
//! [`ModuleKind::RAW_FRONT_END`](crate::model::ModuleKind::RAW_FRONT_END) and its
//! [`RawFrontEnd`] params.
//!
//! It turns the stored sensor samples of a [`SensorData`] into linear Rec.2020 scene-referred
//! RGB (white = 1, **never clipped**), the engine's [`Source`](crate::gpu::Source)
//! (checkpoint C1). Its output size differs from its input (a CFA mosaic, or half size), so
//! it runs before the graph rather than as a graph node: [`Graph::build`](crate::graph::Graph)
//! leaves raw-stage modules to it ([`Graph::front_end`](crate::graph::Graph::front_end)).
//!
//! # Steps
//!
//! 1. **Prepare** (every sample): DNG `FixBadPixelsConstant` (first in `OpcodeList1`), the
//!    other point opcodes of `OpcodeList1` on stored values, the linearization table, black
//!    (per repeat position and channel, plus per-row and per-column deltas), white,
//!    `OpcodeList2`, white balance.
//! 2. **Demosaic**:
//!    - [`Quality::Half`]: 2x2 binning (larger patterns: per-colour means over 4x4).
//!    - [`Quality::Fast`] (Bayer): Malvar–He–Cutler (2004).
//!    - [`Quality::Full`] (Bayer): ratio-corrected directional, after L. Sanz Rodríguez's
//!      RCD (2017): directional discrimination from high-pass energy, green from the four
//!      neighbours each scaled by the ratio of local luminances (the ratio correction) and
//!      weighted by inverse gradients, then red/blue by colour difference along the
//!      diagonals and at green sites. Written from the published description.
//!    - **Dual** ([`Options::dual`], Bayer, full): the directional result where the local
//!      green contrast (four times the range of 3x3 means over a 3x3 window,
//!      over the sum of its ends) is above the threshold,
//!      bilinear interpolation of the mosaic below it, blended by a smooth step from half
//!      to one and a half times the threshold. Flat noisy areas lose the directional
//!      method's maze patterns; edges and texture keep its detail.
//!    - X-Trans (and other 3x3-complete patterns), [`Quality::Fast`] and [`Quality::Full`]:
//!      the four-direction homogeneity method of the CPU path (`specular-raw`'s
//!      `xtrans.rs`, after F. Markesteijn's published technique), ported to floats.
//!      Chosen over a frequency-domain method because X-Trans's 6x6 period puts chroma
//!      carriers close to the luma band, where fixed filters alias or blur; the directional
//!      method keeps detail and is the reference the viewer already ships.
//!    - Linear and monochrome data: none.
//! 3. **`OpcodeList3`** point opcodes on white-balanced camera RGB.
//! 4. **Output**: the camera matrix (`rgb_cam`, then Rec.709 → Rec.2020), or the DNG
//!    profile: camera → ProPhoto, HueSatMap, LookTable (value never clipped), → Rec.2020.
//!
//! Not on the GPU yet (reported in [`FrontEndOutput::skipped`]): `WarpRectilinear` and
//! `WarpFisheye`, `FixBadPixelsList`, and `FixBadPixelsConstant` anywhere but first in
//! list 1. The DNG `ProfileToneCurve` is a display rendering and is left to the view
//! transform.
//!
//! # Bands
//!
//! Every pass runs over horizontal bands of output rows; each band's intermediates cover
//! the band plus the halo of the passes after it, so bands equal a whole-image run exactly
//! (tested). Reads past the image reflect about the edge pixel (Bayer: keeps the phase) or
//! clamp (X-Trans, as the CPU path).

mod correct;
pub(crate) mod data;
pub(crate) mod linear;
pub mod profile_file;
mod shaders;

#[cfg(test)]
pub(crate) mod cpu_ref;
#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use specular_raw::SensorData;
use specular_raw::sensor::Layout;

use crate::gpu::Error;
use crate::graph::Precision;
use crate::model::params::{Demosaic, Highlights, InputProfile, RawFrontEnd};
use crate::ops::correct::{Corrections, HighlightMode};
use shaders::{Pass, WG};

pub use data::{REC709_TO_REC2020, camera_matrix};

/// Uniform bytes per dispatch (the minimum uniform offset alignment WebGPU allows).
const SLOT: u64 = 256;
/// Bytes of intermediates one band may use.
const BAND_BYTES: u64 = 96 << 20;
/// Bytes of full-size pixels one stripe of a reduced run may hold
/// ([`Options::reduce_to`]).
const STRIPE_BYTES: u64 = 64 << 20;
/// Sensor rows read around a sample (bad-pixel fixes).
const RAW_MARGIN: u32 = 2;

/// How much work the demosaic does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Quality {
    /// Half size, 2x2 binning: fit views at scale ≤ ½ and small previews.
    Half,
    /// Malvar–He–Cutler (Bayer); X-Trans uses its full method.
    Fast,
    /// Ratio-corrected directional (Bayer); four-direction homogeneity (X-Trans).
    Full,
}

/// Raw white balance multipliers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WhiteBalance {
    /// The file's (or the grey-world estimate when it has none).
    AsShot,
    /// None (camera RGB as the sensor saw it).
    None,
    /// R, G, B multipliers.
    Multipliers([f32; 3]),
}

/// What the front end outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Output {
    /// Linear Rec.2020 (D65) scene-referred RGB.
    Rec2020,
    /// White-balanced camera RGB (after `OpcodeList3`, before the matrix).
    CameraRgb,
    /// The prepared mosaic in red (crop size): checks and highlight work.
    Mosaic,
}

/// Front-end options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub quality: Quality,
    pub precision: Precision,
    pub wb: WhiteBalance,
    /// Apply the DNG opcode lists.
    pub opcodes: bool,
    /// Use the DNG profile's tables when the file has them.
    pub profile: bool,
    pub output: Output,
    /// Output rows per band (0: from the intermediate budget).
    pub band_rows: u32,
    /// Measure the GPU time (when the device has timestamp queries).
    pub profile_gpu: bool,
    /// Leave `OpcodeList3`'s warps and the `FixVignetteRadial` ops after the first warp to
    /// the geometry warp ([`crate::ops::geometry`]), which applies them in its one sampling
    /// pass; they are then not reported as skipped.
    pub defer_warps: bool,
    /// Dual demosaic (Bayer, [`Quality::Full`]): the contrast threshold below which
    /// bilinear interpolation takes over from the directional method; `None` for none.
    pub dual: Option<f32>,
    /// Highlights, hot pixels, raw chromatic aberration and calibration on the
    /// CFA before demosaic; none by default.
    pub corrections: Corrections,
    /// Wait for the highlight statistics (export, tests); the preview does not
    /// ([`FrontEndStats::stats_pending`]).
    pub wait_stats: bool,
    /// Demosaic at full size and area-reduce to this size (at most the full size) in
    /// stripes, so the full-size picture is never held: views and thumbnails smaller than
    /// the sensor without the false colour of [`Quality::Half`]'s binning. Ignored at
    /// [`Quality::Half`] and for [`Output::Mosaic`].
    pub reduce_to: Option<(u32, u32)>,
    /// Make the highlight statistics at all. Without them clipped highlights take the
    /// neutral grid, as a first run that does not wait for them does (one-off renders
    /// that never refine: edited thumbnails).
    pub stats: bool,
}

impl Options {
    /// Rec.2020 at `quality`, f16 storage, as-shot white balance, opcodes and profile on.
    pub fn new(quality: Quality) -> Options {
        Options {
            quality,
            precision: Precision::F16,
            wb: WhiteBalance::AsShot,
            opcodes: true,
            profile: true,
            output: Output::Rec2020,
            band_rows: 0,
            profile_gpu: false,
            defer_warps: false,
            dual: None,
            corrections: Corrections::default(),
            wait_stats: true,
            reduce_to: None,
            stats: true,
        }
    }

    /// The options a [`RawFrontEnd`] module asks for at view `scale` (≤ ½: half size).
    pub fn from_params(p: &RawFrontEnd, scale: f32) -> Options {
        let quality = if scale <= 0.5 {
            Quality::Half
        } else {
            match p.demosaic {
                Demosaic::Fast => Quality::Fast,
                Demosaic::Directional | Demosaic::Dual => Quality::Full,
            }
        };
        let dual = (p.demosaic == Demosaic::Dual && p.dual_threshold.is_finite())
            .then(|| p.dual_threshold.clamp(0.0, 1.0));
        let corrections = Corrections {
            highlights: match p.highlights {
                Highlights::Opposed => HighlightMode::Opposed,
                Highlights::Rebuild => HighlightMode::Rebuild,
                Highlights::Clip => HighlightMode::Clip,
            },
            highlight_threshold: if p.highlight_threshold.is_finite() {
                p.highlight_threshold.clamp(0.5, 1.0)
            } else {
                1.0
            },
            ..Corrections::default()
        };
        Options {
            opcodes: p.dng_opcodes,
            // File: the profile file's colour stands in for the file's own
            // ([`GpuSensor::use_input_profile`]), tables included.
            profile: p.input_profile != InputProfile::Matrix,
            dual,
            corrections,
            ..Options::new(quality)
        }
    }
}

/// The noise of the front end's output for options `o` (white balance, demosaic or
/// binning, output matrix), from the sensor's raw noise model, with the variance scaled by
/// `reduce` (the area ratio of a later reduction; 1 for none).
pub fn noise_model(
    s: &SensorData,
    raw: &crate::ops::detail::RawNoise,
    o: &Options,
    reduce: f32,
) -> crate::ops::detail::NoiseModel {
    raw.to_working(
        &s.meta.layout,
        o.quality,
        data::wb(&s.meta, o),
        &data::output_matrix(&s.meta),
        reduce,
    )
}

/// Stored samples on the device, ready for [`FrontEnd::run`].
pub struct GpuSensor {
    raw: wgpu::Buffer,
    /// The samples are f32 words (`SensorData::float`).
    float: bool,
    data: SensorData,
    /// Flat-field and dark-frame words, when set.
    cal: Option<wgpu::Buffer>,
    /// The raw chromatic aberration estimate (measured once, on first use).
    ca: std::sync::OnceLock<Option<crate::ops::correct::ca::CaFit>>,
    /// Identity of this upload and of its calibration (statistics cache keys).
    id: u64,
    cal_generation: u64,
    /// The file's own colour (matrix, its source, profile), while an input profile file
    /// stands in for it.
    own_colour: Option<OwnColour>,
}

type OwnColour = (
    [[f32; 3]; 3],
    specular_raw::MatrixSource,
    Option<std::sync::Arc<specular_raw::ProfileTransform>>,
);

impl GpuSensor {
    /// Uses params `p`'s input profile for the colour of the following runs: the file's own
    /// for Auto and Matrix, the profile file for File (the file's own when it cannot be
    /// used, with the reason).
    pub fn use_input_profile(&mut self, p: &RawFrontEnd) -> Option<profile_file::Problem> {
        let m = &mut self.data.meta;
        if let Some((rgb_cam, src, prof)) = self.own_colour.take() {
            m.rgb_cam = rgb_cam;
            m.matrix_source = src;
            m.profile = prof;
        }
        if p.input_profile != InputProfile::File {
            return None;
        }
        let own = (m.rgb_cam, m.matrix_source, m.profile.clone());
        let problem = profile_file::apply(m, p);
        if problem.is_none() {
            self.own_colour = Some(own);
        }
        problem
    }

    /// The sensor (its samples stay on the host as well, for the caller's other uses).
    pub fn sensor(&self) -> &SensorData {
        &self.data
    }

    /// Whether calibration frames are set.
    pub fn calibrated(&self) -> bool {
        self.cal.is_some()
    }

    /// The raw chromatic aberration estimate (Bayer and X-Trans), measured on first use.
    pub fn ca_fit(&self) -> Option<crate::ops::correct::ca::CaFit> {
        *self
            .ca
            .get_or_init(|| crate::ops::correct::ca::estimate_mosaic(&self.data))
    }
}

/// What [`FrontEnd::run`] made.
pub struct FrontEndOutput {
    /// `width × height` pixels, rows packed, at `precision` (rgba32float or packed
    /// rgba16float): pass it to [`Engine::source_from_buffer`](crate::gpu::Engine).
    pub buffer: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
    pub precision: Precision,
    /// Opcodes the file asks for that the GPU path does not apply yet.
    pub skipped: Vec<&'static str>,
    pub stats: FrontEndStats,
}

/// How a run went.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrontEndStats {
    pub bands: usize,
    pub band_rows: u32,
    pub dispatches: usize,
    /// GPU milliseconds of the whole run (when measured).
    pub gpu_ms: Option<f64>,
    /// GPU milliseconds per dispatch, labelled by pass (when measured).
    pub gpu_passes: Vec<(String, f64)>,
    /// Wall milliseconds from recording to the GPU finishing (when measured).
    pub wall_ms: Option<f64>,
    /// Dispatches of the corrections' full-size run (the half-size path has none).
    pub correction_dispatches: usize,
    /// Host milliseconds this run spent on the corrections' statistics (waiting for them
    /// only when [`Options::wait_stats`]).
    pub correction_stats_ms: Option<f64>,
    /// Newer statistics are on their way (the run used older or neutral ones): run again
    /// when [`FrontEnd::stats_ready`].
    pub stats_pending: bool,
}

/// Which buffer a step reads or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Buf {
    None,
    Out,
    /// The mosaic (f32 per pixel).
    M,
    /// vec4 per pixel.
    A,
    B,
    /// Three vec4 per pixel (X-Trans per-line RGB).
    C,
}

impl Buf {
    fn bytes_per_pixel(self) -> u64 {
        match self {
            Buf::M => 4,
            Buf::A | Buf::B => 16,
            Buf::C => 48,
            Buf::None | Buf::Out => 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Step {
    pass: Pass,
    dst: Buf,
    a: Buf,
    b: Buf,
    /// Rows beyond the band this step computes (what the later steps read).
    halo: u32,
}

pub(crate) const fn st(pass: Pass, dst: Buf, a: Buf, b: Buf, halo: u32) -> Step {
    Step {
        pass,
        dst,
        a,
        b,
        halo,
    }
}

/// The steps for a layout, quality and output.
fn steps(layout: &Layout, q: Quality, out: Output, dual: bool) -> Vec<Step> {
    use Buf::*;
    if out == Output::Mosaic {
        return vec![st(Pass::Mosaic, Out, None, None, 0)];
    }
    if q == Quality::Half {
        return vec![st(Pass::Half, Out, None, None, 0)];
    }
    match (layout, q) {
        (Layout::Linear | Layout::Mono, _) => vec![st(Pass::Direct, Out, None, None, 0)],
        (Layout::Bayer(_), Quality::Fast) => vec![
            st(Pass::Prep, M, None, None, 2),
            st(Pass::Mhc, Out, M, None, 0),
        ],
        (Layout::Bayer(_), _) => vec![
            st(Pass::Prep, M, None, None, 13),
            st(Pass::Rcd1, A, M, None, 10),
            st(Pass::Rcd2, B, A, None, 6),
            st(Pass::Rcd3, A, B, None, 3),
            if dual {
                st(Pass::Rcd4d, Out, A, M, 0)
            } else {
                st(Pass::Rcd4, Out, A, None, 0)
            },
        ],
        (Layout::Pattern(_), _) => vec![
            st(Pass::Prep, M, None, None, 8),
            st(Pass::Xt1, A, M, None, 6),
            st(Pass::Xt2, C, M, A, 4),
            st(Pass::Xt3, B, C, None, 3),
            st(Pass::Xt6, Out, B, C, 0),
        ],
    }
}

/// A dispatch before its bind group: pass, uniform words, bindings 1–4 and 6 (offset and
/// size each), workgroups.
#[derive(Clone)]
pub(crate) struct Rec {
    pub(crate) pass: Pass,
    pub(crate) precision: Precision,
    pub(crate) u: [u32; 16],
    pub(crate) bind: [(wgpu::Buffer, u64, u64); 5],
    pub(crate) groups: [u32; 2],
}

/// Where a band run reads its samples: the stored sensor samples, or the corrected mosaic.
#[derive(Clone, Copy)]
pub(crate) struct Src<'a> {
    pub(crate) buf: &'a wgpu::Buffer,
    pub(crate) row_bytes: u64,
    /// Buffer row of crop row 0.
    pub(crate) row0: u32,
    /// Rows in the buffer.
    pub(crate) rows: u32,
    /// Rows read beyond a pixel's own (bad-pixel fixes).
    pub(crate) margin: u32,
    /// The corrected mosaic (f32, crop rows).
    pub(crate) prepared: bool,
    /// Bytes per element of the view's skip (2: samples; 4: words).
    pub(crate) elem: u64,
    pub(crate) crop_w: u32,
    pub(crate) crop_h: u32,
    /// The domain is the mosaic itself, whatever the quality (the correction run).
    pub(crate) mosaic_domain: bool,
}

/// Where a band run writes its output and the bytes per pixel there.
pub(crate) struct Dst<'a> {
    pub(crate) buf: &'a wgpu::Buffer,
    pub(crate) bpp: u64,
    /// The domain rows to make, `[first, end)`, with buffer row 0 holding `first` (a
    /// stripe of a reduced run); `None` for every row, buffer row 0 holding row 0.
    pub(crate) rows: Option<(u32, u32)>,
}

/// One recorded dispatch.
struct Dispatch {
    pipeline: wgpu::ComputePipeline,
    bind: wgpu::BindGroup,
    groups: [u32; 2],
}

/// The raw front end on one device (the app's; it never opens its own).
pub struct FrontEnd {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: HashMap<(Pass, Precision), wgpu::ComputePipeline>,
    dummy: wgpu::Buffer,
    scratch: HashMap<u8, wgpu::Buffer>,
    max_binding: u64,
    max_buffer: u64,
    align: u64,
    timestamps: bool,
    /// The corrected mosaic of the corrections (kept between runs).
    mosaic: Option<wgpu::Buffer>,
    /// The highlight statistics kept and on their way.
    stats: correct::StatsCache,
    /// Milliseconds from starting the last statistics to their tables (GPU and host).
    pub last_stats_ms: Option<f64>,
    /// Pipelines built so far.
    pub pipelines_created: u64,
    /// Run the half-size corrections as three passes (tests compare them with the fused).
    #[cfg(test)]
    pub(crate) unfused_half: bool,
    /// Run the full-size corrections as three passes (tests compare them with the fused).
    #[cfg(test)]
    pub(crate) unfused_full: bool,
    /// Highlight statistics are not looked for in the run that starts them, however fast
    /// the device is (a test then knows its first picture is provisional).
    #[cfg(test)]
    pub(crate) stats_late: bool,
    /// Look 6×6 pattern colours up through `color_at` (tests compare with the packed table).
    #[cfg(test)]
    pub(crate) slow_xtrans: bool,
    /// Full-size rows a stripe of a reduced run holds at most (tests: many stripes).
    #[cfg(test)]
    pub(crate) stripe_rows: Option<u32>,
}

impl FrontEnd {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> FrontEnd {
        let limits = device.limits();
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raw front end"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, ro),
                entry(3, ro),
                entry(4, ro),
                entry(5, ro),
                entry(6, ro),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raw front end"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let dummy = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raw front end dummy"),
            size: 64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        FrontEnd {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipeline_layout,
            pipelines: HashMap::new(),
            dummy,
            scratch: HashMap::new(),
            max_binding: limits
                .max_storage_buffer_binding_size
                .min(limits.max_buffer_size),
            max_buffer: limits.max_buffer_size,
            align: u64::from(limits.min_storage_buffer_offset_alignment)
                .max(u64::from(limits.min_uniform_buffer_offset_alignment))
                .max(16),
            timestamps: device.features().contains(wgpu::Features::TIMESTAMP_QUERY),
            mosaic: None,
            stats: correct::StatsCache::default(),
            last_stats_ms: None,
            pipelines_created: 0,
            #[cfg(test)]
            unfused_half: false,
            #[cfg(test)]
            unfused_full: false,
            #[cfg(test)]
            stats_late: false,
            #[cfg(test)]
            slow_xtrans: false,
            #[cfg(test)]
            stripe_rows: None,
        }
    }

    /// Uploads the stored samples of `s` (two 16-bit samples per word).
    pub fn upload(&mut self, s: SensorData) -> Result<GpuSensor, Error> {
        let n = s.samples.len() as u64;
        let expect = u64::from(s.width) * u64::from(s.height) * u64::from(s.cpp);
        if n == 0 || n != expect {
            return Err(Error::BadRequest(
                "the samples do not match the sensor size",
            ));
        }
        let float = s.float.as_ref().filter(|f| f.len() as u64 == n).cloned();
        let bytes = (n * if float.is_some() { 4 } else { 2 }).div_ceil(16) * 16;
        if bytes > self.max_buffer {
            return Err(Error::TooLarge { needs: bytes });
        }
        let device = &self.device;
        let raw = scoped(device, || {
            Ok(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("raw samples"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }))
        })?;
        // Straight from the samples (no copy of them here); the tail up to the buffer's
        // 16-byte size is zeros.
        let data = match &float {
            Some(f) => bytemuck::cast_slice::<f32, u8>(f),
            None => bytemuck::cast_slice::<u16, u8>(&s.samples),
        };
        let body = data.len() & !3;
        self.queue.write_buffer(&raw, 0, &data[..body]);
        let mut tail = data[body..].to_vec();
        tail.resize(bytes as usize - body, 0);
        if !tail.is_empty() {
            self.queue.write_buffer(&raw, body as u64, &tail);
        }
        Ok(GpuSensor {
            raw,
            float: float.is_some(),
            data: s,
            cal: None,
            ca: std::sync::OnceLock::new(),
            id: correct::next_id(),
            cal_generation: 0,
            own_colour: None,
        })
    }

    fn pipeline(&mut self, pass: Pass, out: Precision) -> Result<wgpu::ComputePipeline, Error> {
        let key = (
            pass,
            if pass.writes_output() {
                out
            } else {
                Precision::F32
            },
        );
        if let Some(p) = self.pipelines.get(&key) {
            return Ok(p.clone());
        }
        let code = shaders::source(pass, key.1);
        let device = &self.device;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(pass.name()),
            source: wgpu::ShaderSource::Wgsl(code.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(pass.name()),
            layout: Some(&self.pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(Error::Gpu(format!("{} does not build: {e}", pass.name())));
        }
        self.pipelines_created += 1;
        self.pipelines.insert(key, pipeline.clone());
        Ok(pipeline)
    }

    /// A scratch buffer for `which` of at least `bytes`.
    fn scratch(&mut self, which: Buf, bytes: u64) -> wgpu::Buffer {
        let k = which as u8;
        if let Some(b) = self.scratch.get(&k)
            && b.size() >= bytes
        {
            return b.clone();
        }
        let b = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raw front end band"),
            size: bytes.div_ceil(256) * 256,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        self.scratch.insert(k, b.clone());
        b
    }

    /// Drops the band buffers (they are kept between runs).
    pub fn release_scratch(&mut self) {
        self.scratch.clear();
        self.mosaic = None;
    }

    /// Output size of `s` at `quality`.
    pub fn output_size(s: &SensorData, quality: Quality, output: Output) -> (u32, u32) {
        let (w, h) = (s.crop.w, s.crop.h);
        if quality == Quality::Half && output != Output::Mosaic {
            ((w / 2).max(1), (h / 2).max(1))
        } else {
            (w, h)
        }
    }

    /// Develops `s` per `o` into a new buffer (see the module documentation).
    pub fn run(&mut self, s: &GpuSensor, o: &Options) -> Result<FrontEndOutput, Error> {
        let device = self.device.clone();
        scoped(&device, || self.run_inner(s, o))
    }

    pub(crate) fn upload_words(&self, words: &[u32]) -> wgpu::Buffer {
        let b = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raw front end data"),
            size: (words.len() as u64 * 4).div_ceil(16) * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&b, 0, bytemuck::cast_slice(words));
        b
    }

    /// A view of rows `[r0, r1)` of a row-major buffer: (offset, size, element skip).
    fn view(
        &self,
        buf: &wgpu::Buffer,
        row_bytes: u64,
        r0: u32,
        r1: u32,
        elem: u64,
    ) -> (u64, u64, u32) {
        if buf.size() <= self.max_binding {
            return (0, buf.size(), 0);
        }
        let first = u64::from(r0) * row_bytes;
        let off = first / self.align * self.align;
        let skip = first - off;
        let size = (skip + u64::from(r1.saturating_sub(r0)) * row_bytes).div_ceil(16) * 16;
        (off, size.min(buf.size() - off), (skip / elem) as u32)
    }

    /// Records `steps` over a `w × h` domain in bands (see the module documentation).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bands(
        &mut self,
        steps: &[Step],
        (w, h): (u32, u32),
        src: &Src,
        dst: &Dst,
        cal: Option<&wgpu::Buffer>,
        o: &Options,
        recs: &mut Vec<Rec>,
    ) -> Result<(usize, u32), Error> {
        let halo = steps[0].halo;
        let px_bytes: u64 = [Buf::M, Buf::A, Buf::B, Buf::C]
            .into_iter()
            .filter(|b| steps.iter().any(|s| s.dst == *b))
            .map(Buf::bytes_per_pixel)
            .sum();
        let row_bytes = u64::from(w) * px_bytes.max(1);
        let biggest = steps
            .iter()
            .map(|s| s.dst.bytes_per_pixel())
            .max()
            .unwrap_or(0)
            .max(dst.bpp)
            * u64::from(w);
        let by_budget = (BAND_BYTES / row_bytes.max(1)).saturating_sub(2 * u64::from(halo));
        let by_binding = (self.max_binding / biggest).saturating_sub(2 * u64::from(halo) + 1);
        let auto = by_budget.min(by_binding).clamp(16, 1 << 20) as u32;
        let (r0, r1) = dst.rows.unwrap_or((0, h));
        let span = r1.saturating_sub(r0).max(1);
        let band = if o.band_rows > 0 { o.band_rows } else { auto }
            .min(span)
            .max(1);
        let bands: Vec<(u32, u32)> = (0..span.div_ceil(band))
            .map(|i| (r0 + i * band, (r0 + (i + 1) * band).min(r1)))
            .collect();
        let buf_rows = u64::from(band + 2 * halo);
        let mut scratch: HashMap<u8, wgpu::Buffer> = HashMap::new();
        for b in [Buf::M, Buf::A, Buf::B, Buf::C] {
            if steps.iter().any(|s| s.dst == b) {
                let bytes = buf_rows * u64::from(w) * b.bytes_per_pixel();
                if bytes > self.max_binding {
                    return Err(Error::TooLarge { needs: bytes });
                }
                scratch.insert(b as u8, self.scratch(b, bytes));
            }
        }
        let dual = o.dual.unwrap_or(0.0);
        let half = o.quality == Quality::Half && o.output != Output::Mosaic && !src.mosaic_domain;
        for &(y0, y1) in &bands {
            let buf_y0 = y0 as i32 - halo as i32;
            for stp in steps {
                let lo = y0.saturating_sub(stp.halo);
                let hi = (y1 + stp.halo).min(h);
                let rows = hi - lo;
                let mut u = [0u32; 16];
                u[0] = lo;
                u[1] = rows;
                u[2] = w;
                u[3] = h;
                u[4] = buf_y0 as u32;
                u[5] = buf_y0 as u32;
                u[11] = dual.to_bits();
                u[12] = u32::from(src.prepared);
                let reads_raw = matches!(
                    stp.pass,
                    Pass::Prep
                        | Pass::CFuse
                        | Pass::Direct
                        | Pass::Mosaic
                        | Pass::Half
                        | Pass::CPrep
                        | Pass::HCell
                        | Pass::HFuse
                        | Pass::HFuseB
                        | Pass::HFuseX
                        | Pass::HFuseBX
                );
                // Crop rows behind domain rows lo..hi (the fused passes also make the rows
                // around them).
                let (rlo, rhi) = if stp.pass.fused() {
                    (lo.saturating_sub(1), (hi + 1).min(h))
                } else if stp.pass == Pass::CFuse {
                    // It prepares three rows around its own.
                    (lo.saturating_sub(3), (hi + 3).min(h))
                } else {
                    (lo, hi)
                };
                let (c0, c1) = if half {
                    ((2 * rlo).saturating_sub(1), (2 * rhi + 2).min(src.crop_h))
                } else {
                    (rlo, rhi)
                };
                let raw = if reads_raw {
                    let r0 = (src.row0 + c0).saturating_sub(src.margin);
                    let r1 = (src.row0 + c1 + src.margin).min(src.rows);
                    let (off, size, skip) = self.view(src.buf, src.row_bytes, r0, r1, src.elem);
                    if off == 0 && size == src.buf.size() {
                        u[10] = src.rows;
                    } else {
                        u[8] = skip;
                        u[9] = r0;
                        u[10] = r1 - r0;
                    }
                    (src.buf.clone(), off, size)
                } else {
                    (self.dummy.clone(), 0, 64)
                };
                let calv = match cal {
                    Some(cb)
                        if matches!(
                            stp.pass,
                            Pass::CPrep
                                | Pass::CFuse
                                | Pass::HCell
                                | Pass::HFuse
                                | Pass::HFuseB
                                | Pass::HFuseX
                                | Pass::HFuseBX
                        ) =>
                    {
                        let cw = u64::from(src.crop_w) * 4;
                        let (off, size, skip) = self.view(cb, cw, c0, c1, 4);
                        if !(off == 0 && size == cb.size()) {
                            u[13] = skip;
                            u[14] = c0;
                        }
                        (cb.clone(), off, size)
                    }
                    _ => (self.dummy.clone(), 0, 64),
                };
                let dv = if stp.dst == Buf::Out {
                    // The output step makes the band's own rows (no halo).
                    let first = u64::from(lo - r0) * u64::from(w) * dst.bpp;
                    let off = first / self.align * self.align;
                    let skip = first - off;
                    let size = (skip + u64::from(rows) * u64::from(w) * dst.bpp).div_ceil(16) * 16;
                    let size = size.min(dst.buf.size() - off);
                    u[6] = lo;
                    u[7] = (skip / dst.bpp) as u32;
                    (dst.buf.clone(), off, size)
                } else {
                    u[6] = buf_y0 as u32;
                    u[7] = 0;
                    let b = scratch[&(stp.dst as u8)].clone();
                    let size = b.size();
                    (b, 0, size)
                };
                let sv = |b: Buf| match b {
                    Buf::None | Buf::Out => (self.dummy.clone(), 0, 64),
                    _ => {
                        let s = scratch[&(b as u8)].clone();
                        let n = s.size();
                        (s, 0, n)
                    }
                };
                recs.push(Rec {
                    pass: stp.pass,
                    precision: o.precision,
                    u,
                    bind: [dv, sv(stp.a), sv(stp.b), raw, calv],
                    groups: [w.div_ceil(WG), rows.div_ceil(WG)],
                });
            }
        }
        Ok((bands.len(), band))
    }

    /// Builds the bind groups of `recs` and records them in one compute pass.
    pub(crate) fn encode(
        &mut self,
        recs: &[Rec],
        dbuf: &wgpu::Buffer,
        enc: &mut wgpu::CommandEncoder,
        qs: Option<&wgpu::QuerySet>,
    ) -> Result<(), Error> {
        let n = recs.len().max(1);
        let ubuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raw front end params"),
            size: n as u64 * SLOT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut uniforms = vec![0u32; n * (SLOT as usize / 4)];
        for (i, r) in recs.iter().enumerate() {
            uniforms[i * 64..i * 64 + 16].copy_from_slice(&r.u);
        }
        self.queue
            .write_buffer(&ubuf, 0, bytemuck::cast_slice(&uniforms));
        let mut ds = Vec::with_capacity(recs.len());
        for (i, r) in recs.iter().enumerate() {
            let pipeline = self.pipeline(r.pass, r.precision)?;
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: range(&ubuf, i as u64 * SLOT, 64),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: range(dbuf, 0, dbuf.size()),
                },
            ];
            for (k, (b, off, size)) in r.bind.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: [1, 2, 3, 4, 6][k],
                    resource: range(b, *off, *size),
                });
            }
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("raw front end"),
                layout: &self.layout,
                entries: &entries,
            });
            ds.push(Dispatch {
                pipeline,
                bind,
                groups: r.groups,
            });
        }
        // Timed: a pass per dispatch, its start and end at queries 2i and 2i + 1.
        if let Some(q) = qs {
            for (i, d) in ds.iter().enumerate() {
                let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("raw front end"),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                        query_set: q,
                        beginning_of_pass_write_index: Some(2 * i as u32),
                        end_of_pass_write_index: Some(2 * i as u32 + 1),
                    }),
                });
                cp.set_pipeline(&d.pipeline);
                cp.set_bind_group(0, &d.bind, &[]);
                cp.dispatch_workgroups(d.groups[0].max(1), d.groups[1].max(1), 1);
            }
            return Ok(());
        }
        let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("raw front end"),
            timestamp_writes: None,
        });
        for d in &ds {
            cp.set_pipeline(&d.pipeline);
            cp.set_bind_group(0, &d.bind, &[]);
            cp.dispatch_workgroups(d.groups[0].max(1), d.groups[1].max(1), 1);
        }
        Ok(())
    }

    /// Records `steps` over the full-size `fw × fh` domain in stripes of rows, each
    /// followed by an area reduction ([`Pass::Reduce`], the weights of the preview's
    /// reduction) into the `dw × dh` output `out`: the full-size pixels never exist
    /// at once. Returns the bands recorded and the stripe's rows.
    #[allow(clippy::too_many_arguments)]
    fn reduced_bands(
        &mut self,
        steps: &[Step],
        (fw, fh): (u32, u32),
        (dw, dh): (u32, u32),
        src: &Src,
        out: &wgpu::Buffer,
        cal: Option<&wgpu::Buffer>,
        o: &Options,
        recs: &mut Vec<Rec>,
    ) -> Result<(usize, u32), Error> {
        // The stripe holds full-size pixels at the output's precision (as a full-size run
        // would): f16 halves what the passes write and the reduction reads.
        let px = o.precision.bytes_per_pixel();
        let row_bytes = u64::from(fw) * px;
        let cap = (STRIPE_BYTES.min(self.max_binding) / row_bytes).max(4);
        let cap = u32::try_from(cap).unwrap_or(u32::MAX);
        #[cfg(test)]
        let cap = self.stripe_rows.unwrap_or(cap);
        let sy = f64::from(fh) / f64::from(dh);
        // Full-size rows output rows `[d0, d1)` read (a row of margin for f32 rounding
        // in the kernel).
        let need = |d0: u32, d1: u32| -> (u32, u32) {
            let s0 = (f64::from(d0) * sy).floor() as u32;
            let s1 = (f64::from(d1) * sy).ceil() as u32;
            (s0.saturating_sub(1), (s1 + 1).min(fh))
        };
        let per = ((f64::from(cap.saturating_sub(3)) / sy).floor() as u32).max(1);
        let stripe_rows = (0..dh)
            .step_by(per as usize)
            .map(|d0| {
                let (s0, s1) = need(d0, (d0 + per).min(dh));
                s1 - s0
            })
            .max()
            .unwrap_or(1);
        let stripe = self.scratch_out(u64::from(stripe_rows) * row_bytes)?;
        let mut nbands = 0;
        let mut band = 0;
        let out_bpp = o.precision.bytes_per_pixel();
        for d0 in (0..dh).step_by(per as usize) {
            let d1 = (d0 + per).min(dh);
            let (s0, s1) = need(d0, d1);
            let (n, b) = self.bands(
                steps,
                (fw, fh),
                src,
                &Dst {
                    buf: &stripe,
                    bpp: px,
                    rows: Some((s0, s1)),
                },
                cal,
                o,
                recs,
            )?;
            nbands += n;
            band = band.max(b);
            let mut u = [0u32; 16];
            u[0] = d0;
            u[1] = d1 - d0;
            u[2] = dw;
            u[3] = dh;
            u[4] = s0;
            u[8] = fw;
            u[9] = fh;
            u[13] = s1 - s0;
            let first = u64::from(d0) * u64::from(dw) * out_bpp;
            let off = first / self.align * self.align;
            let skip = first - off;
            let size = (skip + u64::from(d1 - d0) * u64::from(dw) * out_bpp).div_ceil(16) * 16;
            let size = size.min(out.size() - off);
            u[6] = d0;
            u[7] = (skip / out_bpp) as u32;
            let dummy = (self.dummy.clone(), 0, 64);
            recs.push(Rec {
                pass: Pass::Reduce,
                precision: o.precision,
                u,
                bind: [
                    (out.clone(), off, size),
                    (stripe.clone(), 0, stripe.size()),
                    dummy.clone(),
                    dummy.clone(),
                    dummy,
                ],
                groups: [dw.div_ceil(WG), (d1 - d0).div_ceil(WG)],
            });
        }
        Ok((nbands, band))
    }

    /// The stripe of full-size output pixels of a reduced run (kept between runs).
    fn scratch_out(&mut self, bytes: u64) -> Result<wgpu::Buffer, Error> {
        if bytes > self.max_binding {
            return Err(Error::TooLarge { needs: bytes });
        }
        Ok(self.scratch(Buf::Out, bytes))
    }

    /// The half-size steps with corrections (raw CA or not).
    fn half_steps(&self, ca: bool, xtrans: bool) -> Vec<Step> {
        #[cfg(test)]
        if self.unfused_half {
            return correct::half_steps_unfused();
        }
        correct::half_steps(ca, xtrans)
    }

    fn run_inner(&mut self, s: &GpuSensor, o: &Options) -> Result<FrontEndOutput, Error> {
        let sd = &s.data;
        let meta = &sd.meta;
        if sd.crop.w == 0
            || sd.crop.h == 0
            || sd.crop.x + sd.crop.w > sd.width
            || sd.crop.y + sd.crop.h > sd.height
        {
            return Err(Error::BadRequest("the crop is outside the sensor"));
        }
        let geo = data::Geometry {
            width: sd.width,
            height: sd.height,
            cpp: sd.cpp,
            crop: sd.crop,
            float: s.float,
        };
        let (mut words, skipped) = data::build(meta, geo, o);
        #[cfg(test)]
        if self.slow_xtrans {
            words[data::H_XT6_ON] = 0;
        }
        let elem: u64 = if s.float { 4 } else { 2 };
        let t0 = Instant::now();
        let raw_src = Src {
            buf: &s.raw,
            row_bytes: u64::from(sd.width) * u64::from(sd.cpp) * elem,
            row0: sd.crop.y,
            rows: sd.height,
            margin: RAW_MARGIN,
            prepared: false,
            elem,
            crop_w: sd.crop.w,
            crop_h: sd.crop.h,
            mosaic_domain: false,
        };
        // The corrected mosaic, when any correction runs.
        let corrected = if correct::applies(sd, o) {
            Some(self.correct(s, o, &raw_src, &mut words)?)
        } else {
            None
        };
        let dbuf = self.upload_words(&words);
        let mut recs = Vec::new();
        if let Some(c) = &corrected {
            recs.extend(c.recs.iter().cloned());
        }
        let f_src;
        let src = match corrected.as_ref().and_then(|c| c.mosaic.as_ref()) {
            Some(mosaic) => {
                f_src = Src {
                    buf: mosaic,
                    row_bytes: u64::from(sd.crop.w) * 4,
                    row0: 0,
                    rows: sd.crop.h,
                    margin: 0,
                    prepared: true,
                    elem: 4,
                    ..raw_src
                };
                &f_src
            }
            None => &raw_src,
        };
        // The half-size path corrects the half-size pixels themselves.
        let half_corrected = corrected.as_ref().is_some_and(|c| c.mosaic.is_none());
        let steps = if half_corrected {
            self.half_steps(
                corrected.as_ref().is_some_and(|c| c.ca),
                matches!(meta.layout, Layout::Pattern(_)),
            )
        } else {
            steps(
                &meta.layout,
                o.quality,
                o.output,
                o.dual.unwrap_or(0.0) > 0.0,
            )
        };
        let cal = s
            .cal
            .as_ref()
            .filter(|_| half_corrected && o.corrections.calibrate);
        let (fw, fh) = Self::output_size(sd, o.quality, o.output);
        // A reduced run: the full-size demosaic in stripes, each area-reduced into the
        // output as it is made (only a stripe of full-size pixels at a time).
        let reduce = o
            .reduce_to
            .filter(|_| o.quality != Quality::Half && o.output != Output::Mosaic)
            .map(|(dw, dh)| (dw.clamp(1, fw), dh.clamp(1, fh)))
            .filter(|&r| r != (fw, fh));
        let (w, h) = reduce.unwrap_or((fw, fh));
        let out_bpp = o.precision.bytes_per_pixel();
        let out_bytes = u64::from(w) * u64::from(h) * out_bpp;
        if out_bytes > self.max_buffer {
            return Err(Error::TooLarge { needs: out_bytes });
        }
        let device = &self.device;
        let out = scoped(device, || {
            Ok(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("raw front end output"),
                size: out_bytes.div_ceil(16) * 16,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }))
        })?;
        let first = recs.len();
        let (nbands, band) = match reduce {
            None => self.bands(
                &steps,
                (w, h),
                src,
                &Dst {
                    buf: &out,
                    bpp: out_bpp,
                    rows: None,
                },
                cal,
                o,
                &mut recs,
            )?,
            Some(d) => self.reduced_bands(&steps, (fw, fh), d, src, &out, cal, o, &mut recs)?,
        };
        let dispatches = recs.len() - first;
        let profile = o.profile_gpu && self.timestamps;
        let nq = 2 * (recs.len() as u32).max(1);
        let qb = u64::from(nq) * 8;
        let qs = profile.then(|| {
            self.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("raw front end"),
                ty: wgpu::QueryType::Timestamp,
                count: nq,
            })
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("raw front end"),
            });
        self.encode(&recs, &dbuf, &mut enc, qs.as_ref())?;
        let read = qs.as_ref().map(|q| {
            let resolve = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("raw front end profile"),
                size: qb,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("raw front end profile read"),
                size: qb,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            enc.resolve_query_set(q, 0..nq, &resolve, 0);
            enc.copy_buffer_to_buffer(&resolve, 0, &read, 0, qb);
            read
        });
        self.queue.submit([enc.finish()]);
        let mut stats = FrontEndStats {
            bands: nbands,
            band_rows: band,
            dispatches,
            ..FrontEndStats::default()
        };
        if let Some(c) = &corrected {
            stats.correction_dispatches = c.recs.len();
            stats.correction_stats_ms = c.stats_ms;
            stats.stats_pending = c.pending;
        }
        if o.profile_gpu {
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| Error::Gpu(e.to_string()))?;
            stats.wall_ms = Some(t0.elapsed().as_secs_f64() * 1e3);
            if let Some(read) = read {
                let t: Vec<u64> = {
                    let v = map_read(&self.device, &read, qb)?;
                    bytemuck::pod_collect_to_vec(&v[..])
                };
                read.unmap();
                let period = f64::from(self.queue.get_timestamp_period()) * 1e-6;
                stats.gpu_passes = recs
                    .iter()
                    .zip(t.chunks_exact(2))
                    .map(|(r, q)| {
                        (
                            format!("raw {:?}", r.pass),
                            q[1].saturating_sub(q[0]) as f64 * period,
                        )
                    })
                    .collect();
                stats.gpu_ms = Some(stats.gpu_passes.iter().map(|p| p.1).sum());
            }
        }
        Ok(FrontEndOutput {
            buffer: out,
            width: w,
            height: h,
            precision: o.precision,
            skipped,
            stats,
        })
    }

    /// Reads an output back as f32 RGBA (tests and tools).
    pub fn read(&self, o: &FrontEndOutput) -> Result<Vec<f32>, Error> {
        let bytes = u64::from(o.width) * u64::from(o.height) * o.precision.bytes_per_pixel();
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raw front end read"),
            size: bytes.div_ceil(4) * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("raw front end read"),
            });
        enc.copy_buffer_to_buffer(&o.buffer, 0, &staging, 0, bytes);
        self.queue.submit([enc.finish()]);
        let v = {
            let view = map_read(&self.device, &staging, bytes)?;
            match o.precision {
                Precision::F32 => bytemuck::pod_collect_to_vec::<u8, f32>(&view[..]),
                Precision::F16 => bytemuck::pod_collect_to_vec::<u8, u16>(&view[..])
                    .into_iter()
                    .map(|h| half::f16::from_bits(h).to_f32())
                    .collect(),
            }
        };
        staging.unmap();
        Ok(v)
    }
}

fn range(b: &wgpu::Buffer, off: u64, size: u64) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer: b,
        offset: off,
        size: std::num::NonZeroU64::new(size),
    })
}

/// Runs `f` inside validation and out-of-memory error scopes.
fn scoped<T>(device: &wgpu::Device, f: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    let oom = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let val = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let r = f();
    let e1 = pollster::block_on(val.pop());
    let e2 = pollster::block_on(oom.pop());
    match (e1, e2) {
        (Some(e), _) | (None, Some(e)) => Err(Error::Gpu(e.to_string())),
        _ => r,
    }
}

fn map_read(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    bytes: u64,
) -> Result<wgpu::BufferView, Error> {
    let (tx, rx) = mpsc::channel();
    buffer
        .slice(..bytes)
        .map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r.is_ok());
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| Error::Gpu(e.to_string()))?;
    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(true) => {}
        _ => return Err(Error::Gpu("reading the result back failed".into())),
    }
    buffer
        .slice(..bytes)
        .get_mapped_range()
        .map_err(|e| Error::Gpu(format!("{e:?}")))
}
