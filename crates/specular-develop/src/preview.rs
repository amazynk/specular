//! The interactive preview: one image
//! developed at about screen size on the app's GPU and written into a texture the UI draws.
//!
//! - **Source (checkpoint C1).** A raw goes through the raw front end ([`crate::ops::raw`])
//!   once per front-end setting and size: half-size binning when the screen needs at most
//!   half the sensor, the full demosaic otherwise; then an area-weighted reduction to the
//!   requested long side. Other images arrive as linear Rec.2020 pixels ([`Input::Rgb`]),
//!   already reduced by the caller: they enter at the scene-linear stage. A crop develops
//!   the source larger, so the crop still gets the requested size.
//! - **Geometry (checkpoint C3).** The fused warp ([`crate::ops::geometry`]) turns the
//!   source by [`Developer::base`] and the edit's turns, straightens, crops and corrects the
//!   lens in one pass; its output is kept until the geometry or the source changes.
//!   Denoise runs before it, on the unwarped source over the region the warp reads
//!   ([`crate::prewarp`], checkpoint C2), so the warp never stretches the noise.
//! - **Render.** The edit's graph plus the output transform ([`OutputSpace`]) runs on the
//!   engine ([`crate::gpu::Engine`]) in f16 storage. A slider rewrites uniforms only.
//! - **Half-scale drafts.** While a slider is dragged (a module in focus) on a picture of at
//!   least [`HALF_DRAFT_PIXELS`] and an op that reads neighbours runs at or after the focus,
//!   the graph runs at half view scale on a half-size copy of the picture and is scaled up;
//!   the render without focus refines. Denoise
//!   then runs after the warp. Modules with drafts of their own (diffuse, surface blur,
//!   retouch, depth ops) and a shown mask keep the full scale.
//! - **Present.** A small pass turns the result (turned by [`Orient`]) into sRGB-encoded
//!   8-bit rows and copies them into an `Rgba8UnormSrgb` texture: nothing is read back to
//!   the host.
//! - **Newest change wins.** [`Developer::idle`] says when the GPU has finished what was
//!   submitted; the caller submits the newest edit only then, so changes never queue up.
//!
//! Without a usable hardware GPU the same code runs on the software adapter
//! ([`open_software_device`]); the caller then reads the result back with
//! [`Developer::read_rgba8`] and uploads it to its own device.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use specular_raw::SensorData;

use crate::clip::{ClipFlags, ClipParams, ClipPass};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Error, Rendered, Request, Source};
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::mask::{Geometry, MaskCache, MaskEnv, Overlay};
use crate::model::params::{ModuleParams, RawFrontEnd};
use crate::model::{Edit, ModuleKind, Params};
use crate::ops::detail::{ImageDetail, NoiseModel, RawNoise, with_image};
use crate::ops::geometry::warp::Warper;
use crate::ops::geometry::{Context, Plan};
use crate::ops::output::OutputSpace;
use crate::ops::raw::{FrontEnd, GpuSensor, Options, Output, Quality};
use crate::proof::{ProofLut, ProofPass};
use crate::scopes::{Bins, ScopeEngine};

/// How the developed image is turned on screen (the file's orientation and the user's
/// quarter turns), applied when presenting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Orient {
    /// Rows become columns.
    pub transpose: bool,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Orient {
    /// The turn that shows an image stored with EXIF orientation `o` (1..=8) upright.
    pub fn from_exif(o: u16) -> Orient {
        let (transpose, flip_x, flip_y) = match o {
            2 => (false, true, false),
            3 => (false, true, true),
            4 => (false, false, true),
            5 => (true, false, false),
            6 => (true, true, false),
            7 => (true, true, true),
            8 => (true, false, true),
            _ => (false, false, false),
        };
        Orient {
            transpose,
            flip_x,
            flip_y,
        }
    }

    /// Output size of a `w`×`h` image.
    pub fn size(&self, w: u32, h: u32) -> (u32, u32) {
        if self.transpose { (h, w) } else { (w, h) }
    }

    /// The stored pixel shown at output (x, y) of a `w`×`h` stored image.
    pub fn source_of(&self, x: u32, y: u32, w: u32, h: u32) -> (u32, u32) {
        let (ow, oh) = self.size(w, h);
        let dx = if self.flip_x { ow - 1 - x } else { x };
        let dy = if self.flip_y { oh - 1 - y } else { y };
        if self.transpose { (dy, dx) } else { (dx, dy) }
    }
}

/// [`Developer::ai_picture`]: the picture, the plan onto the frame (None: it is the
/// frame), the frame's size in sensor pixels.
pub type AiPicture = (crate::mask::ai::Picture, Option<Plan>, (f64, f64));

/// [`Developer::upright_lines`]: what automatic upright solves from.
#[derive(Clone, Debug, PartialEq)]
pub struct UprightLines {
    /// Centred oriented-frame pixels.
    pub lines: Vec<crate::ops::geometry::upright::Line>,
    /// The oriented frame's size (square pixels).
    pub oriented: (f64, f64),
    /// The crop module's straighten angle (degrees).
    pub straighten: f32,
    /// The file's 35 mm equivalent focal length.
    pub focal35: Option<f64>,
}

/// What the preview develops.
#[derive(Clone)]
pub enum Input {
    /// Sensor data from `specular-raw`'s `decode_sensor`.
    Raw(Box<SensorData>),
    /// Linear Rec.2020 (D65) RGBA, `width × height × 4` values, already at about screen
    /// size.
    Rgb {
        width: u32,
        height: u32,
        rgba: Vec<f32>,
    },
}

/// The as-shot illuminant of a raw (CIE 1931 xy), when the file says enough to know it:
/// the DNG `AsShotWhiteXY`, or `AsShotNeutral` through the inverse colour matrix (the
/// second illuminant's, usually D65, else the first). Other raws give `None`; their
/// as-shot white stays the placeholder.
pub fn as_shot_white(s: &SensorData) -> Option<[f64; 2]> {
    use crate::colour::{inv, mulv, valid_xy, xyz_to_xy};
    let d = s.meta.dng.as_ref()?;
    if let Some((x, y)) = d.as_shot_white_xy
        && valid_xy([x, y])
    {
        return Some([x, y]);
    }
    let n = d.as_shot_neutral?;
    let cm = d.color_matrix[1].or(d.color_matrix[0])?;
    let m: crate::colour::M3 = std::array::from_fn(|i| std::array::from_fn(|j| cm[i * 3 + j]));
    let xyz = mulv(&inv(&m)?, n);
    let xy = xyz_to_xy(xyz);
    valid_xy(xy).then_some(xy)
}

/// [`as_shot_white`], and for other raws (`data` the file) the as-shot multipliers the
/// file records (maker notes) through the body's colour matrix
/// ([`specular_raw::as_shot_xy`]). `None` (the placeholder stays) when the multipliers were
/// estimated or the body has no table matrix.
pub fn as_shot_white_of(s: &SensorData, data: &[u8]) -> Option<[f64; 2]> {
    if s.meta.dng.is_some() {
        return as_shot_white(s);
    }
    if s.meta.wb_source != specular_raw::WbSource::AsShot {
        return None;
    }
    let f = specular_raw::parse(data).ok()?;
    let i = f.info();
    specular_raw::as_shot_xy(&i.make, &i.model, s.meta.wb).filter(|xy| crate::colour::valid_xy(*xy))
}

/// The software adapter's device (lavapipe, WARP), for machines whose GPU cannot run
/// Develop.
pub fn open_software_device() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.flags = wgpu::InstanceFlags::empty().with_env();
    desc.backends = wgpu::Backends::PRIMARY.with_env();
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: true,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("no software adapter: {e}"))?;
    let a = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(a.clone());
    limits.max_storage_buffer_binding_size = a.max_storage_buffer_binding_size;
    limits.max_buffer_size = a.max_buffer_size;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("develop (software)"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("cannot open the software adapter: {e}"))
}

/// Drafts of pictures with at least this many pixels may render at half view scale
/// ([`Developer::half_drafts`]).
pub const HALF_DRAFT_PIXELS: u64 = 1_500_000;

/// How much larger than the view a raw's source is made for a warp that resamples
/// ([`Developer::warped_source`]), at most the sensor's size.
const OVERSAMPLE: f64 = 2.0;
/// ... and at least this long side, so small pictures (thumbnails) warp near full size:
/// at a large reduction, a warp of a twice-size picture still resamples aliased detail.
const OVERSAMPLE_LONG: f64 = 2048.0;

/// Modules whose drafts are their own (placed drafts, kept scratch): never halved.
const OWN_DRAFTS: [ModuleKind; 5] = [
    ModuleKind::DIFFUSE,
    ModuleKind::SURFACE_BLUR,
    ModuleKind::RETOUCH,
    ModuleKind::RELIGHT,
    ModuleKind::DEPTH_BLUR,
];

/// Threads per workgroup side of the helper passes.
const WG: u32 = 16;

const AUX_WGSL: &str =
    "struct U { sw: u32, sh: u32, dw: u32, dh: u32, prec: u32, flags: u32, stride: u32, pad: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<storage, read> src: array<u32>;
fn load(i: u32) -> vec4<f32> {
    if (u.prec == 1u) {
        return vec4<f32>(unpack2x16float(src[2u * i]), unpack2x16float(src[2u * i + 1u]));
    }
    return vec4<f32>(bitcast<f32>(src[4u * i]), bitcast<f32>(src[4u * i + 1u]),
        bitcast<f32>(src[4u * i + 2u]), bitcast<f32>(src[4u * i + 3u]));
}
@compute @workgroup_size(16, 16, 1)
fn reduce(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= u.dw || gid.y >= u.dh) { return; }
    let sx = f32(u.sw) / f32(u.dw);
    let sy = f32(u.sh) / f32(u.dh);
    let x0 = f32(gid.x) * sx;
    let x1 = x0 + sx;
    let y0 = f32(gid.y) * sy;
    let y1 = y0 + sy;
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    let ye = min(u32(ceil(y1)), u.sh);
    let xe = min(u32(ceil(x1)), u.sw);
    for (var y = u32(floor(y0)); y < ye; y++) {
        let wy = min(f32(y + 1u), y1) - max(f32(y), y0);
        for (var x = u32(floor(x0)); x < xe; x++) {
            let wx = min(f32(x + 1u), x1) - max(f32(x), x0);
            let w = wx * wy;
            if (w > 0.0) {
                acc += load(y * u.sw + x) * w;
                wsum += w;
            }
        }
    }
    let v = acc / max(wsum, 1e-12);
    let o = gid.y * u.dw + gid.x;
    dst[2u * o] = pack2x16float(v.xy);
    dst[2u * o + 1u] = pack2x16float(v.zw);
}
@compute @workgroup_size(16, 16, 1)
fn upscale(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= u.dw || gid.y >= u.dh) { return; }
    let fx = clamp((f32(gid.x) + 0.5) * f32(u.sw) / f32(u.dw) - 0.5, 0.0, f32(u.sw - 1u));
    let fy = clamp((f32(gid.y) + 0.5) * f32(u.sh) / f32(u.dh) - 0.5, 0.0, f32(u.sh - 1u));
    let x0 = u32(fx);
    let y0 = u32(fy);
    let x1 = min(x0 + 1u, u.sw - 1u);
    let y1 = min(y0 + 1u, u.sh - 1u);
    let ax = fx - f32(x0);
    let ay = fy - f32(y0);
    let top = mix(load(y0 * u.sw + x0), load(y0 * u.sw + x1), ax);
    let bot = mix(load(y1 * u.sw + x0), load(y1 * u.sw + x1), ax);
    let v = mix(top, bot, ay);
    let o = gid.y * u.dw + gid.x;
    if (u.prec == 1u) {
        dst[2u * o] = pack2x16float(v.xy);
        dst[2u * o + 1u] = pack2x16float(v.zw);
        return;
    }
    dst[4u * o] = bitcast<u32>(v.x);
    dst[4u * o + 1u] = bitcast<u32>(v.y);
    dst[4u * o + 2u] = bitcast<u32>(v.z);
    dst[4u * o + 3u] = bitcast<u32>(v.w);
}
fn srgb(c: f32) -> f32 {
    let l = clamp(c, 0.0, 1.0);
    if (l <= 0.0031308) { return l * 12.92; }
    return 1.055 * pow(l, 1.0 / 2.4) - 0.055;
}
@compute @workgroup_size(16, 16, 1)
fn present(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= u.dw || gid.y >= u.dh) { return; }
    var x = gid.x;
    var y = gid.y;
    if ((u.flags & 2u) != 0u) { x = u.dw - 1u - x; }
    if ((u.flags & 4u) != 0u) { y = u.dh - 1u - y; }
    var sx = x;
    var sy = y;
    if ((u.flags & 1u) != 0u) { sx = y; sy = x; }
    let v = load(sy * u.sw + sx);
    dst[gid.y * u.stride + gid.x] = pack4x8unorm(vec4<f32>(srgb(v.x), srgb(v.y), srgb(v.z), 1.0));
}
@compute @workgroup_size(16, 16, 1)
fn present_hdr(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= u.dw || gid.y >= u.dh) { return; }
    var x = gid.x;
    var y = gid.y;
    if ((u.flags & 2u) != 0u) { x = u.dw - 1u - x; }
    if ((u.flags & 4u) != 0u) { y = u.dh - 1u - y; }
    var sx = x;
    var sy = y;
    if ((u.flags & 1u) != 0u) { sx = y; sy = x; }
    let v = clamp(load(sy * u.sw + sx).rgb, vec3<f32>(0.0), vec3<f32>(65000.0));
    let o = 2u * (gid.y * u.stride + gid.x);
    dst[o] = pack2x16float(v.xy);
    dst[o + 1u] = pack2x16float(vec2<f32>(v.z, 1.0));
}
@compute @workgroup_size(16, 16, 1)
fn tint(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= u.dw || gid.y >= u.dh) { return; }
    let i = gid.y * u.dw + gid.x;
    let k = 0.5 * clamp(load(i).x, 0.0, 1.0);
    let red = vec3<f32>(1.0, 0.0, 0.0);
    if (u.prec == 1u) {
        let c = vec3<f32>(unpack2x16float(dst[2u * i]), unpack2x16float(dst[2u * i + 1u]).x);
        let t = mix(c, red, k);
        dst[2u * i] = pack2x16float(t.xy);
        dst[2u * i + 1u] = pack2x16float(vec2<f32>(t.z, 1.0));
        return;
    }
    let c = vec3<f32>(bitcast<f32>(dst[4u * i]), bitcast<f32>(dst[4u * i + 1u]),
        bitcast<f32>(dst[4u * i + 2u]));
    let t = mix(c, red, k);
    dst[4u * i] = bitcast<u32>(t.x);
    dst[4u * i + 1u] = bitcast<u32>(t.y);
    dst[4u * i + 2u] = bitcast<u32>(t.z);
    dst[4u * i + 3u] = bitcast<u32>(1.0);
}
";

/// The reduction and present passes.
struct Aux {
    layout: wgpu::BindGroupLayout,
    reduce: wgpu::ComputePipeline,
    /// Bilinear enlargement of a half-scale draft to the picture's size.
    upscale: wgpu::ComputePipeline,
    present: wgpu::ComputePipeline,
    /// Linear half floats for an `Rgba16Float` texture (HDR, values above 1 kept).
    present_hdr: wgpu::ComputePipeline,
    /// Tints `dst` red by the grey mask in `src` (the overlay drawn apart).
    tint: wgpu::ComputePipeline,
}

impl Aux {
    fn new(device: &wgpu::Device) -> Aux {
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
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("develop preview"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop preview"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop preview"),
            source: wgpu::ShaderSource::Wgsl(AUX_WGSL.into()),
        });
        let pipe = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("develop preview"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Aux {
            reduce: pipe("reduce"),
            upscale: pipe("upscale"),
            present: pipe("present"),
            present_hdr: pipe("present_hdr"),
            tint: pipe("tint"),
            layout,
        }
    }
}

/// What the current source was made from.
#[derive(Clone, Debug, PartialEq)]
struct SourceKey {
    front: Option<Params>,
    long_side: u32,
    /// The raw corrections and the calibration frames set (by generation).
    corrections: crate::ops::correct::Corrections,
    calibration: u64,
    /// The geometry warp resamples: a raw's refined source is also made at up to twice
    /// the size for it ([`Developer::warped_source`]).
    oversample: bool,
    /// [`Developer::size_multiple`].
    multiple: u32,
}

/// What the denoised unwarped source was made from ([`crate::prewarp`]).
#[derive(Clone, Copy, Debug, PartialEq)]
struct PreKey {
    source: u64,
    graph: crate::model::hash::Hash,
    scale: u32,
}

/// What the warped source was made from.
#[derive(Clone, Debug, PartialEq)]
struct WarpKey {
    source: u64,
    geometry: Vec<Option<Params>>,
    base: Orient,
    crop_on: bool,
    size: (u32, u32),
}

/// Timings of the last preparation and render (wall clock on the host; GPU time when
/// profiled).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewStats {
    /// Front end + reduction, wall ms (submission only unless profiled).
    pub prepare_ms: f64,
    pub front_end_gpu_ms: Option<f64>,
    /// Graph GPU ms of the last render, when profiled.
    pub render_gpu_ms: Option<f64>,
    pub pipelines_created: u64,
    /// The last render was a half-scale draft ([`HALF_DRAFT_PIXELS`]) or a smaller one
    /// ([`Developer::drag_level`]).
    pub half_draft: bool,
    /// The divisor of the view's sides the last render was drawn at (1: the view's size).
    pub draft_divisor: u32,
    /// The last render left sharpening out ([`Developer::defer_sharpen`]).
    pub sharpen_deferred: bool,
}

/// One image developed for the screen.
pub struct Developer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    engine: Engine,
    front: FrontEnd,
    aux: Aux,
    sensor: Option<GpuSensor>,
    rgb: Option<Source>,
    source: Option<Source>,
    key: Option<SourceKey>,
    busy: Arc<AtomicBool>,
    staging: Option<wgpu::Buffer>,
    warper: Warper,
    warped: Option<(WarpKey, Source)>,
    /// Denoise's result on the unwarped source, over the region the warp reads.
    prewarped: Option<(PreKey, Roi, Source)>,
    /// A raw's (or a larger image's) source at up to twice the size (by the id of the
    /// source made from it), that
    /// the geometry warp samples before an area reduction: a warp of the reduced picture
    /// would resample detail the reduction already aliased.
    oversampled: Option<(u64, Source)>,
    /// The reduced copy of a picture (by its id and divisor) that reduced drafts start
    /// from.
    half: Option<(u64, u32, Source)>,
    /// Drafts of large pictures render at half view scale (on by default).
    pub half_drafts: bool,
    /// The level a module in focus is drafted at ([`crate::drag::Pace`], "Preview while
    /// dragging"); `None`: half scale for large pictures with an op that reads neighbours
    /// at or after the focus ([`Developer::drag_default`]). Modules with drafts of their
    /// own and a shown mask keep the full scale either way.
    pub drag_level: Option<crate::drag::Level>,
    /// Frame times of renders ([`Developer::time_frames`], [`Developer::take_frame_ms`]).
    clock: Option<FrameClock>,
    /// Refined pictures are kept in the engine's cache, so undo and redo copy them (on by
    /// default).
    pub keep_results: bool,
    /// Refined renders keep every group's output as a checkpoint
    /// ([`Request::checkpoint_every`]); on by default.
    pub checkpoint_every: bool,
    /// The first picture of a new source is a half-scale draft (when it would be one for a
    /// slider: large, an op that reads neighbours); [`Developer::refine_due`] then asks
    /// for the refined one. Off by default (one-off renders such as thumbnails).
    pub draft_first: bool,
    /// The current source has not been rendered yet.
    fresh: bool,
    /// The last picture was a first-frame draft: the refined one is due.
    draft_shown: bool,
    /// A refined picture after a change leaves sharpening out (the rest is the
    /// refined picture's); [`Developer::refine_due`] then asks for the whole one, which
    /// resumes at sharpening. Off by default (one-off renders); the app turns it on.
    pub defer_sharpen: bool,
    /// The graph (its last group's hash) whose picture went out without sharpening: the
    /// next refined render of it is the whole one.
    sharpen_deferred: Option<crate::model::hash::Hash>,
    /// The current source is a raw binned at half size ([`Quality::Half`], a first
    /// picture with [`Developer::draft_first`]): the full demosaic reduced is due.
    binned_draft: bool,
    /// [`Developer::refine_due`] asked for the refined source: the next
    /// [`Developer::prepare`] makes it.
    refine_source: bool,
    /// The last render had a module in focus (a drag): the binned source stays.
    dragging: bool,
    /// The refined source develops again (newer highlight statistics): it stays refined.
    keep_refined: bool,
    /// Denoise after the warp, as before `crate::prewarp` (timing comparisons).
    #[cfg(test)]
    pub(crate) denoise_after_warp: bool,
    /// The orientation the image is shown in before the edit's own turns (the file's): the
    /// warp applies it, so renders come out upright.
    pub base: Orient,
    /// Apply the crop (off while the crop is being edited: the whole straightened frame).
    pub crop_on: bool,
    /// The image's facts for the detail ops (noise of the current source, lens blur).
    detail: ImageDetail,
    /// The sensor's noise model (fitted on first use) and the front-end options and
    /// reduction of the current source, to carry it to the source's units.
    raw_noise: Option<RawNoise>,
    noise_of: Option<(Options, f32)>,
    /// Calibration frames set on the sensor (counts each change).
    calibration: u64,
    /// A non-raw image reduced for the RGB chromatic aberration estimate (until made), and
    /// the estimate (percent of the radius, red and blue).
    rgb_ca_src: Option<(u32, u32, Vec<f32>)>,
    rgb_ca: Option<Option<[f32; 2]>>,
    /// The current source used older or neutral highlight statistics (newer ones are on
    /// their way).
    provisional: bool,
    /// Measure GPU time (blocks on each render: benchmarks only).
    pub profile: bool,
    /// Make the raw highlight statistics (on by default). One-off renders that never
    /// refine turn it off: they would draw with the neutral grid anyway.
    pub highlight_stats: bool,
    /// Wait for the highlight statistics instead of drawing with the last or a neutral
    /// grid (one-off renders such as edited thumbnails: they never refine).
    pub wait_highlight_stats: bool,
    /// Sensor pixels per sample of the raw given (2 for a sensor binned for a thumbnail,
    /// [`specular_raw::SensorData::half`]): neighbourhood ops keep their sensor-pixel
    /// scales.
    pub sensor_bin: f32,
    /// The source is made a whole multiple of this times the size it would be at
    /// `long_side / size_multiple` (thumbnails: a picture reduced by a whole factor equals
    /// one reduction from the sensor; at a factor like 1.5 every other output pixel
    /// straddles two, and the two reductions blur more than one). 1 for views.
    pub size_multiple: u32,
    /// Render the picture in tiles of this many pixels a side (tests: tile seams in the
    /// 1:1 view); `None` tiles only when the budget asks.
    pub tile_side: Option<u32>,
    pub stats: PreviewStats,
    /// Brush rasters and the mask coordinate grid of the current image.
    masks: MaskCache,
    /// Show this module's mask over the picture.
    pub mask_overlay: Option<((ModuleKind, u16), Overlay)>,
    /// The darkroom aids: scopes, soft-proof and clipping passes, made on first use.
    scopes: Option<ScopeEngine>,
    proofing: Option<ProofPass>,
    clipping: Option<ClipPass>,
    /// The next image, developed ahead ([`Developer::prefetch`]).
    staged: Option<Staged>,
    /// The Lensfun lookup of the current raw ([`Developer::set_lensfun`]).
    lensfun: Option<Arc<crate::ops::geometry::lensfun::Shot>>,
    /// The file's 35 mm equivalent focal length ([`Developer::set_focal35`]).
    focal35: Option<f64>,
    /// Why the raw front end's profile file is not used, when it is asked for and is not.
    input_profile_problem: Option<crate::ops::raw::profile_file::Problem>,
}

/// One image's state on a [`Developer`]: the image shown lives in the developer's fields;
/// one developed ahead waits here ([`Developer::prefetch`]).
#[derive(Default)]
struct Image {
    sensor: Option<GpuSensor>,
    rgb: Option<Source>,
    source: Option<Source>,
    key: Option<SourceKey>,
    detail: ImageDetail,
    raw_noise: Option<RawNoise>,
    noise_of: Option<(Options, f32)>,
    rgb_ca_src: Option<(u32, u32, Vec<f32>)>,
    rgb_ca: Option<Option<[f32; 2]>>,
    provisional: bool,
    fresh: bool,
}

/// An image developed ahead: the caller's tag, the image, what it was developed for.
struct Staged {
    tag: u64,
    image: Image,
    edit: Edit,
    long_side: u32,
    base: Orient,
}

impl Developer {
    /// A developer on `device` (the app's, or the software adapter's).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, budget: Budget) -> Developer {
        Developer {
            device: device.clone(),
            queue: queue.clone(),
            engine: Engine::new(device, queue, budget),
            front: FrontEnd::new(device, queue),
            aux: Aux::new(device),
            sensor: None,
            rgb: None,
            source: None,
            key: None,
            busy: Arc::new(AtomicBool::new(false)),
            staging: None,
            warper: Warper::new(device, queue),
            warped: None,
            prewarped: None,
            half: None,
            half_drafts: true,
            drag_level: None,
            clock: None,
            keep_results: true,
            checkpoint_every: true,
            draft_first: false,
            fresh: false,
            draft_shown: false,
            defer_sharpen: false,
            sharpen_deferred: None,
            binned_draft: false,
            refine_source: false,
            dragging: false,
            keep_refined: false,
            oversampled: None,
            #[cfg(test)]
            denoise_after_warp: false,
            base: Orient::default(),
            crop_on: true,
            detail: ImageDetail::default(),
            raw_noise: None,
            noise_of: None,
            calibration: 0,
            rgb_ca_src: None,
            rgb_ca: None,
            provisional: false,
            profile: false,
            lensfun: None,
            focal35: None,
            input_profile_problem: None,
            highlight_stats: true,
            wait_highlight_stats: false,
            sensor_bin: 1.0,
            size_multiple: 1,
            tile_side: None,
            stats: PreviewStats::default(),
            masks: MaskCache::default(),
            mask_overlay: None,
            scopes: None,
            proofing: None,
            clipping: None,
            staged: None,
        }
    }

    /// Sets the image (forgets the previous one and its cached checkpoints).
    pub fn set_input(&mut self, input: Input) -> Result<(), Error> {
        self.staged = None;
        self.forget_image();
        self.load(input)
    }

    /// Forgets the image's checkpoints and state (not one developed ahead).
    fn forget_image(&mut self) {
        let mut gone = Image::default();
        self.swap_image(&mut gone);
        self.warped = None;
        self.prewarped = None;
        self.half = None;
        self.oversampled = None;
        self.binned_draft = false;
        self.refine_source = false;
        self.keep_refined = false;
        self.warper.release();
        self.engine.forget();
        self.front.release_scratch();
        self.masks.clear();
    }

    /// Swaps the image's own state with `other`.
    fn swap_image(&mut self, other: &mut Image) {
        use std::mem::swap;
        swap(&mut self.sensor, &mut other.sensor);
        swap(&mut self.rgb, &mut other.rgb);
        swap(&mut self.source, &mut other.source);
        swap(&mut self.key, &mut other.key);
        swap(&mut self.detail, &mut other.detail);
        swap(&mut self.raw_noise, &mut other.raw_noise);
        swap(&mut self.noise_of, &mut other.noise_of);
        swap(&mut self.rgb_ca_src, &mut other.rgb_ca_src);
        swap(&mut self.rgb_ca, &mut other.rgb_ca);
        swap(&mut self.provisional, &mut other.provisional);
        swap(&mut self.fresh, &mut other.fresh);
    }

    /// Develops the next image ahead: uploads `input` and
    /// makes its source for `edit` at `long_side`, turned by `base`, now, apart from the
    /// image shown (whose checkpoints stay). [`Developer::take_prefetched`] with the same
    /// `tag` then makes it the image at no front-end cost. `noise`: the sensor's noise
    /// model, fitted off this thread ([`RawNoise::of_sensor`]). One image is kept; a new
    /// call or [`Developer::drop_prefetched`] lets it go. Returns false (nothing kept)
    /// when it would hold more than [`Budget::kept`] of the device's memory, or while the
    /// image shown still waits for its highlight statistics (try again later).
    #[allow(clippy::too_many_arguments)]
    pub fn prefetch(
        &mut self,
        tag: u64,
        input: Input,
        noise: Option<RawNoise>,
        edit: &Edit,
        long_side: u32,
        base: Orient,
    ) -> Result<bool, Error> {
        self.staged = None;
        // The image shown waits for its own highlight statistics: they go first (one
        // read-back at a time).
        if self.refining() {
            return Ok(false);
        }
        let input_bytes = match &input {
            Input::Raw(s) => s.samples.len() as u64 * if s.float.is_some() { 4 } else { 2 },
            Input::Rgb { rgba, .. } => rgba.len() as u64 * 4,
        };
        // The source is at most `long_side` square in f16, plus the front end's output.
        let bytes = input_bytes + u64::from(long_side).pow(2) * 8 * 2;
        if bytes > self.engine.budget().kept() {
            return Ok(false);
        }
        let mut image = Image::default();
        self.swap_image(&mut image);
        let shown = (self.base, self.crop_on);
        self.base = base;
        self.crop_on = true;
        let r = self.load(input).and_then(|()| {
            if noise.is_some() {
                self.raw_noise = noise;
            }
            self.develop(edit, long_side).map(|_| ())
        });
        (self.base, self.crop_on) = shown;
        self.swap_image(&mut image);
        r?;
        self.staged = Some(Staged {
            tag,
            image,
            edit: edit.clone(),
            long_side,
            base,
        });
        Ok(true)
    }

    /// The tag of the image developed ahead, if any.
    pub fn prefetched(&self) -> Option<u64> {
        self.staged.as_ref().map(|s| s.tag)
    }

    /// Lets the image developed ahead go (the user went elsewhere).
    pub fn drop_prefetched(&mut self) {
        self.staged = None;
    }

    /// Develops the image developed ahead again once its highlight statistics have arrived
    /// (they were on their way when it was made), so it is shown final at once. Call when
    /// the GPU is free; true when it ran.
    pub fn refine_prefetched(&mut self) -> Result<bool, Error> {
        let Some(st) = self.staged.as_mut() else {
            return Ok(false);
        };
        let ready = st.image.provisional
            && st
                .image
                .sensor
                .as_ref()
                .is_some_and(|s| self.front.stats_ready_of(s));
        if !ready {
            return Ok(false);
        }
        let Some(mut st) = self.staged.take() else {
            return Ok(false);
        };
        self.swap_image(&mut st.image);
        let shown = (self.base, self.crop_on);
        self.base = st.base;
        self.crop_on = true;
        self.key = None;
        let r = self.develop(&st.edit, st.long_side);
        (self.base, self.crop_on) = shown;
        self.swap_image(&mut st.image);
        r?;
        self.staged = Some(st);
        Ok(true)
    }

    /// Makes the image developed ahead with `tag` the image (as [`Developer::set_input`]
    /// would, its source already made); false when there is none with that tag.
    pub fn take_prefetched(&mut self, tag: u64) -> bool {
        if self.staged.as_ref().is_none_or(|s| s.tag != tag) {
            return false;
        }
        let Some(mut st) = self.staged.take() else {
            return false;
        };
        self.forget_image();
        self.swap_image(&mut st.image);
        // Its statistics went to another image meanwhile: develop it again (they restart).
        if self.provisional
            && self
                .sensor
                .as_ref()
                .is_some_and(|s| !self.front.stats_pending_of(s))
        {
            self.key = None;
        }
        true
    }

    /// Sets the raw's noise model fitted off this thread ([`RawNoise::of_sensor`] of the
    /// same sensor); otherwise denoise fits it on first use.
    pub fn set_raw_noise(&mut self, noise: Option<RawNoise>) {
        if self.sensor.is_some() && noise.is_some() {
            self.raw_noise = noise;
        }
    }

    /// Uploads `input` as the image (its state forgotten first).
    fn load(&mut self, input: Input) -> Result<(), Error> {
        match input {
            Input::Raw(s) => self.sensor = Some(self.front.upload(*s)?),
            Input::Rgb {
                width,
                height,
                rgba,
            } => {
                self.detail.noise = crate::ops::detail::noise::estimate_rgb(width, height, &rgba);
                self.rgb = Some(self.engine.upload(width, height, &rgba)?);
                self.rgb_ca_src = Some(reduce_rgba(width, height, rgba, 2048));
            }
        }
        Ok(())
    }

    /// Sets (or clears) the flat-field and dark-frame calibration of a raw
    /// ([`crate::ops::correct::calib::for_edit`] makes it); the source develops again.
    pub fn set_calibration(
        &mut self,
        cal: Option<&crate::ops::correct::calib::Calibration>,
    ) -> Result<(), Error> {
        if let Some(s) = self.sensor.as_mut() {
            // Nothing to change (a prefetched source stays).
            if cal.is_none() && !s.calibrated() {
                return Ok(());
            }
            self.front.set_calibration(s, cal)?;
            self.calibration += 1;
        }
        Ok(())
    }

    /// The RGB chromatic aberration of a non-raw image: estimated once (percent of the
    /// radius), or the module's manual scales.
    fn rgb_ca(&mut self, edit: &Edit) -> Option<[f32; 2]> {
        let p = crate::ops::correct::chromatic_aberration(edit)?;
        if let Some(s) = &self.sensor {
            // Bayer raws are corrected on the CFA; other layouts (X-Trans) take the manual
            // scales in the warp (no estimate yet).
            let bayer = matches!(
                s.sensor().meta.layout,
                specular_raw::sensor::Layout::Bayer(_)
            );
            return (!bayer && !p.auto).then_some([p.red, p.blue]);
        }
        if !p.auto {
            return Some([p.red, p.blue]);
        }
        if self.rgb_ca.is_none() {
            let est = self.rgb_ca_src.take().and_then(|(w, h, v)| {
                crate::ops::correct::ca::estimate_rgb(w, h, &v).map(|k| k.map(|k| k * 100.0))
            });
            self.rgb_ca = Some(est);
        }
        self.rgb_ca.flatten()
    }

    /// Whether the picture will refine: newer highlight statistics are on their way, or the
    /// source is binned and nothing is dragged ([`Developer::refine_due`]).
    pub fn refining(&self) -> bool {
        self.sensor
            .as_ref()
            .is_some_and(|s| self.front.stats_pending_of(s))
            || (self.binned_draft && !self.dragging)
    }

    /// The front end, for tests that set its test-only switches.
    #[cfg(test)]
    pub(crate) fn front_mut(&mut self) -> &mut FrontEnd {
        &mut self.front
    }

    /// Whether the current source was developed with older or neutral highlight statistics
    /// (its clipped highlights may show the wrong colour until [`Developer::refine_due`]).
    /// A caller that can keep showing an earlier picture should hold this one back.
    pub fn provisional(&self) -> bool {
        self.provisional && self.rgb.is_none()
    }

    /// Whether the picture should render again: the last one was a first-frame draft
    /// ([`Developer::draft_first`]), or highlight statistics have arrived since it (the
    /// source then develops again with them on the next [`Developer::prepare`]). Never
    /// waits.
    pub fn refine_due(&mut self) -> bool {
        let mut due = std::mem::take(&mut self.draft_shown);
        // The binned first source gives way to the full demosaic reduced once nothing is
        // dragged (the binning puts red and blue off centre: false colour on edges).
        if self.binned_draft && !self.refine_source && !self.dragging && self.source.is_some() {
            self.refine_source = true;
            due = true;
        }
        if due {
            return true;
        }
        let ready = match &self.sensor {
            Some(s) => self.front.stats_ready_of(s),
            None => false,
        };
        if ready {
            self.keep_refined = !self.binned_draft;
            self.key = None;
        }
        ready
    }

    /// Sets the lens of the image (aperture and pixel pitch, when known): capture
    /// sharpening's automatic radius.
    /// The Lensfun lookup of the current raw (`None`: none, or the raw has data of its own;
    /// [`crate::ops::geometry::lensfun::shot_of_raw`]). Set with each new source.
    pub fn set_lensfun(&mut self, shot: Option<Arc<crate::ops::geometry::lensfun::Shot>>) {
        if self.lensfun != shot {
            self.lensfun = shot;
            self.warped = None;
            self.prewarped = None;
        }
    }

    /// The file's 35 mm equivalent focal length, for perspective
    /// ([`crate::ops::geometry::persp::focal35_of_file`]). Set with each new source.
    pub fn set_focal35(&mut self, f: Option<f64>) {
        if self.focal35 != f {
            self.focal35 = f;
            self.warped = None;
            self.prewarped = None;
        }
    }

    pub fn set_lens(&mut self, f_number: Option<f32>, pitch_um: Option<f32>) {
        self.detail.capture_sigma =
            Some(crate::ops::detail::capture::auto_sigma(f_number, pitch_um));
    }

    /// The noise model of the current source: the sensor's (fitted once, on first use)
    /// carried through the front end and the reduction.
    fn source_noise(&mut self) -> Option<NoiseModel> {
        if let Some(rgb) = &self.rgb {
            // Estimated on the image as given: an area reduction averages it down.
            let k = self.source.as_ref().map_or(1.0, |s| {
                (f64::from(s.width) * f64::from(s.height)
                    / (f64::from(rgb.width) * f64::from(rgb.height))) as f32
            });
            return self.detail.noise.map(|n| n.scaled(k.min(1.0)));
        }
        let (o, reduce) = self.noise_of?;
        let s = self.sensor.as_ref()?.sensor();
        let raw = *self.raw_noise.get_or_insert_with(|| RawNoise::of_sensor(s));
        Some(crate::ops::raw::noise_model(s, &raw, &o, reduce))
    }

    /// The sensor of a raw input.
    pub fn sensor(&self) -> Option<&SensorData> {
        self.sensor.as_ref().map(GpuSensor::sensor)
    }

    /// Watches the device's lost flag ([`Engine::watch_device_loss`]).
    pub fn watch_device_loss(&mut self, flag: Arc<AtomicBool>) {
        self.engine.watch_device_loss(flag);
    }

    /// Whether the device was lost: nothing renders on it any more.
    pub fn device_lost(&self) -> bool {
        self.engine.device_lost()
    }

    /// Gives back what only speeds up the next render ([`Engine::trim_memory`]), when the
    /// system is short of memory.
    pub fn trim_memory(&mut self) {
        self.engine.trim_memory();
    }

    /// Whether everything submitted has finished on the GPU (a lost device's work never
    /// will: idle, so the next call reports the loss).
    pub fn idle(&self) -> bool {
        if self.engine.device_lost() {
            return true;
        }
        if crate::gpu::poll(&self.device, wgpu::PollType::Poll) == Err(Error::DeviceLost) {
            self.engine.mark_lost();
            return true;
        }
        !self.busy.load(Ordering::Acquire)
    }

    /// Size of the developed source (before turning).
    pub fn source_size(&self) -> Option<(u32, u32)> {
        self.source.as_ref().map(|s| (s.width, s.height))
    }

    /// Makes the source for `edit` with its long side at most `long_side` (a raw is
    /// developed again only when its front-end params or the size change). Returns its size.
    pub fn prepare(&mut self, edit: &Edit, long_side: u32) -> Result<(u32, u32), Error> {
        if self.develop(edit, long_side)? {
            self.engine.forget();
            self.warped = None;
            self.prewarped = None;
            self.half = None;
        }
        let s = self.source.as_ref().ok_or(Error::BadRequest("no image"))?;
        Ok((s.width, s.height))
    }

    /// Makes the source for `edit` (see [`Developer::prepare`]) unless the one there fits;
    /// true when it made a new one.
    fn develop(&mut self, edit: &Edit, long_side: u32) -> Result<bool, Error> {
        let mut long_side = long_side.max(16);
        // A crop shows part of the source: develop the source larger so the crop still gets
        // `long_side` pixels (in half-octave steps, so dragging a crop rarely re-develops).
        let whole = match (&self.sensor, &self.rgb) {
            (Some(s), _) => {
                let c = s.sensor().crop;
                Some((c.w.max(c.h).max(1), Context::raw(s.sensor(), self.base)))
            }
            (None, Some(r)) => Some((
                r.width.max(r.height).max(1),
                Context::image(r.width, r.height, self.base),
            )),
            (None, None) => None,
        };
        if let Some((full, mut ctx)) = whole {
            // Only the crop's size matters here: leave the lens out (no fill zoom to fit).
            ctx.lens = None;
            ctx.list3.clear();
            let plan = Plan::new(edit, &ctx, self.crop_on);
            let need = plan.scale_for(long_side) * f64::from(full);
            let mut l = f64::from(long_side);
            while l < need - 0.5 && l < f64::from(full) {
                l *= std::f64::consts::SQRT_2;
            }
            long_side = (l.round() as u32).min(full).max(long_side.min(full));
        }
        let front = edit
            .modules
            .iter()
            .find(|m| m.kind == ModuleKind::RAW_FRONT_END && m.enabled)
            .map(|m| m.params.clone());
        let mut corrections = crate::ops::correct::Corrections::of_edit(edit);
        corrections.calibrate &= self.sensor.as_ref().is_some_and(GpuSensor::calibrated);
        let key = SourceKey {
            front: front.clone(),
            long_side,
            corrections,
            calibration: self.calibration,
            oversample: self.warp_resamples(edit),
            multiple: self.size_multiple.max(1),
        };
        let same = self.key.as_ref() == Some(&key);
        let refining = same && self.binned_draft && self.refine_source;
        if same && !refining && self.source.is_some() {
            return Ok(false);
        }
        // A new source on screen starts binned when that is quicker and the caller shows
        // drafts first; otherwise (and once `refine_due` asked) the full demosaic reduced.
        let binned = self.draft_first && !refining && !std::mem::take(&mut self.keep_refined);
        let t0 = std::time::Instant::now();
        let mut was_binned = false;
        let source = if let Some(rgb) = self.rgb.clone() {
            // Other images are area-reduced to the view (in linear light, as raws are).
            let (w, h) = (rgb.width, rgb.height);
            let (dw, dh) = fit_multiple(w, h, long_side, key.multiple);
            self.oversampled = None;
            if (dw, dh) == (w, h) {
                rgb
            } else {
                let buf = self.reduce(rgb.buffer(), w, h, rgb.precision, dw, dh)?;
                let s = self
                    .engine
                    .source_from_buffer(buf, dw, dh, Precision::F16)?;
                // A warp that resamples samples the image at up to twice the view (then
                // reduces), as for raws.
                if key.oversample {
                    let want = (OVERSAMPLE * f64::from(long_side)).max(OVERSAMPLE_LONG);
                    let (bw, bh) = fit(w, h, want.round() as u32);
                    let big = if (bw, bh) == (w, h) {
                        rgb
                    } else {
                        let buf = self.reduce(rgb.buffer(), w, h, rgb.precision, bw, bh)?;
                        self.engine
                            .source_from_buffer(buf, bw, bh, Precision::F16)?
                    };
                    self.oversampled = Some((s.id(), big));
                }
                s
            }
        } else if self.sensor.is_some() {
            let p = match &front {
                Some(Params::RawFrontEnd(p)) => p.clone(),
                Some(other) => RawFrontEnd::from_fields(&other.to_fields()),
                None => RawFrontEnd::default(),
            };
            if let Some(s) = self.sensor.as_mut() {
                self.input_profile_problem = s.use_input_profile(&p);
            }
            let Some(sensor) = &self.sensor else {
                return Err(Error::BadRequest("no image"));
            };
            let (cw, ch) = (sensor.sensor().crop.w, sensor.sensor().crop.h);
            let scale = long_side as f32 / cw.max(ch).max(1) as f32;
            let full = FrontEnd::output_size(sensor.sensor(), Quality::Fast, Output::Rec2020);
            let mut o = if binned || scale > 0.5 {
                Options::from_params(&p, scale)
            } else {
                Options::from_params(&p, 1.0)
            };
            was_binned = o.quality == Quality::Half;
            // A warp that resamples samples a source up to twice the size (then reduces).
            let over = if !was_binned && key.oversample {
                let want = (OVERSAMPLE * f64::from(long_side)).max(OVERSAMPLE_LONG);
                want.min(f64::from(full.0.max(full.1))) / f64::from(long_side)
            } else {
                1.0
            };
            if !was_binned {
                // The full demosaic area-reduced in stripes (never held at full size).
                let long = (f64::from(long_side) * over).round() as u32;
                o.reduce_to = Some(if over > 1.0 {
                    fit(full.0, full.1, long.max(long_side))
                } else {
                    fit_multiple(full.0, full.1, long_side, key.multiple)
                });
            }
            // Highlights follow the front end's params; the other corrections their modules.
            o.corrections = crate::ops::correct::Corrections {
                highlights: o.corrections.highlights,
                highlight_threshold: o.corrections.highlight_threshold,
                ..corrections
            };
            o.precision = Precision::F16;
            // The highlight statistics never block the screen: the first picture uses the
            // last (or a neutral) grid and refines when they arrive (`refine_due`).
            o.wait_stats = self.wait_highlight_stats;
            o.stats = self.highlight_stats;
            o.profile_gpu = self.profile;
            o.defer_warps = true;
            let out = self.front.run(sensor, &o)?;
            self.stats.front_end_gpu_ms = out.stats.gpu_ms;
            self.provisional = out.stats.stats_pending;
            let (w, h) = (out.width, out.height);
            let (dw, dh) = fit_multiple(w, h, long_side, key.multiple);
            // The noise model's area ratio from what the front end made before reducing.
            let (mw, mh) = if was_binned { (w, h) } else { full };
            let reduce = (f64::from(dw) * f64::from(dh) / (f64::from(mw) * f64::from(mh))) as f32;
            self.noise_of = Some((o, reduce.min(1.0)));
            self.oversampled = None;
            if (dw, dh) == (w, h) {
                self.engine
                    .source_from_buffer(out.buffer, w, h, out.precision)?
            } else {
                let buf = self.reduce(&out.buffer, w, h, out.precision, dw, dh)?;
                let s = self
                    .engine
                    .source_from_buffer(buf, dw, dh, Precision::F16)?;
                if key.oversample && !was_binned {
                    let big = self
                        .engine
                        .source_from_buffer(out.buffer, w, h, out.precision)?;
                    self.oversampled = Some((s.id(), big));
                }
                s
            }
        } else {
            return Err(Error::BadRequest("no image"));
        };
        self.stats.prepare_ms = t0.elapsed().as_secs_f64() * 1e3;
        // The refined source of a picture already shown is not a first picture.
        self.fresh = !refining;
        self.binned_draft = was_binned && binned;
        self.refine_source = false;
        self.source = Some(source);
        self.key = Some(key);
        Ok(true)
    }

    /// The fused warp of `edit` on the current image as the picture shows it (crop as
    /// [`Developer::crop_on`] says): maps between the picture and the source frame, where
    /// masks live ([`Plan::to_source`], [`Plan::to_output`] at scale 1 over
    /// [`Plan::crop_rect`]).
    pub fn plan(&self, edit: &Edit) -> Option<Plan> {
        let src = self.source.as_ref()?;
        Some(Plan::new(edit, &self.context(src), self.crop_on))
    }

    /// The masks' view of the picture `src` (the warp's output) for `edit`.
    fn mask_geometry(&self, edit: &Edit, unwarped: &Source) -> Option<Plan> {
        if !crate::mask::has_blends(edit)
            && !crate::ops::retouch::needs_geometry(edit)
            && !crate::ops::detail::ai::needs_geometry(edit)
        {
            return None;
        }
        let plan = Plan::new(edit, &self.context(unwarped), self.crop_on);
        (!plan.is_identity()).then_some(plan)
    }

    /// The graph of `edit` with its masks on `src` (the picture after the warp).
    #[allow(clippy::too_many_arguments)]
    fn graph(
        &mut self,
        edit: &Edit,
        unwarped: &Source,
        src: &Source,
        focus: Option<(ModuleKind, u16)>,
        out: Option<&OutputSpace>,
        scale: f32,
        overlay: Option<((ModuleKind, u16), Overlay)>,
    ) -> Result<Graph, Error> {
        let plan = self.mask_geometry(edit, unwarped);
        let ctx = self.context(unwarped);
        let geometry = match &plan {
            Some(p) => Geometry::Warp(p),
            None => Geometry::Frame {
                w: ctx.frame[2],
                h: ctx.frame[3],
            },
        };
        let mut env = MaskEnv {
            geometry,
            image: (src.width, src.height),
            scale,
            cache: &mut self.masks,
            sensor_bin: self.sensor_bin.max(1.0) as f64,
            overlay,
        };
        Graph::build_masked(edit, OpRegistry::builtin(), focus, out, Some(&mut env)).map_err(|e| {
            Error::Unsupported(match e {
                crate::graph::GraphError::ParamsTooLarge(n) => n,
            })
        })
    }

    /// Whether `edit`'s geometry warp resamples a raw (lens distortion or colour fringes,
    /// the file's own lens data, a straighten angle): not for quarter turns, crops alone
    /// or non-raw inputs (given at the size the caller wants).
    fn warp_resamples(&self, edit: &Edit) -> bool {
        let sensor = self.sensor.as_ref().map(GpuSensor::sensor);
        if sensor.is_none() && self.rgb.is_none() {
            return false;
        }
        let Some(sensor) = sensor else {
            return rgb_warp_resamples(edit);
        };
        let lens = crate::ops::geometry::lens_params(edit).is_some_and(|l| {
            l.distortion != 0.0
                || l.tca_red != 0.0
                || l.tca_blue != 0.0
                || (l.embedded
                    && (sensor.meta.lens.is_some()
                        || self
                            .lensfun
                            .as_deref()
                            .is_some_and(|s| s.chosen(&l.lensfun).is_some())
                        || crate::ops::geometry::dng_steps(&sensor.meta.opcodes[2])
                            .iter()
                            .any(|d| matches!(d, crate::ops::geometry::DngStep::Warp { .. }))))
        });
        lens || crate::ops::geometry::crop_params(edit).angle_deg != 0.0 || persp_resamples(edit)
    }

    /// The warp's view of the current input.
    fn context(&self, src: &Source) -> Context {
        match &self.sensor {
            Some(s) => Context {
                lensfun: self.lensfun.clone(),
                focal35: self.focal35,
                ..Context::raw(s.sensor(), self.base)
            },
            None => Context {
                focal35: self.focal35,
                ..Context::image(src.width, src.height, self.base)
            },
        }
    }

    /// The picture the graph of `nodes` starts from: `src` through `geom`'s warp, with
    /// denoise run before the warp when it resamples ([`crate::prewarp`]). Returns it and
    /// the edit to build the graph from (`nodes` without denoise when denoise ran here).
    /// Denoise stays in the graph while its mask is shown (the overlay travels the graph).
    fn picture(
        &mut self,
        geom: &Edit,
        nodes: &Edit,
        src: &Source,
        overlay: Option<((ModuleKind, u16), Overlay)>,
    ) -> Result<(Source, Option<Edit>), Error> {
        let shown = overlay.is_some_and(|((k, _), _)| k == ModuleKind::DENOISE);
        #[cfg(test)]
        let shown = shown || self.denoise_after_warp;
        let split = if shown {
            None
        } else {
            crate::prewarp::split(nodes)
        };
        let Some((before, after)) = split else {
            return Ok((self.warped_source(geom, src)?, None));
        };
        let mut ctx = self.context(src);
        ctx.rgb_ca = self.rgb_ca(geom);
        let plan = Plan::new(geom, &ctx, self.crop_on);
        if !crate::prewarp::wanted(&plan) {
            return Ok((self.warped_source(geom, src)?, None));
        }
        let base = self.denoised(&before, src, &plan)?;
        Ok((self.warped_source(geom, &base)?, Some(after)))
    }

    /// `before` (denoise and what precedes it) rendered on the unwarped `src` over the
    /// region `plan`'s warp reads, in a buffer of `src`'s size; kept while the source, the
    /// graph and the region stay.
    fn denoised(&mut self, before: &Edit, src: &Source, plan: &Plan) -> Result<Source, Error> {
        let scale = self.view_scale(src);
        let ctx = self.context(src);
        let mut env = MaskEnv {
            geometry: Geometry::Frame {
                w: ctx.frame[2],
                h: ctx.frame[3],
            },
            image: (src.width, src.height),
            scale,
            cache: &mut self.masks,
            sensor_bin: self.sensor_bin.max(1.0) as f64,
            overlay: None,
        };
        let graph = Graph::build_masked(before, OpRegistry::builtin(), None, None, Some(&mut env))
            .map_err(|e| {
                Error::Unsupported(match e {
                    crate::graph::GraphError::ParamsTooLarge(n) => n,
                })
            })?;
        let Some(last) = graph.groups.len().checked_sub(1) else {
            return Ok(src.clone());
        };
        let key = PreKey {
            source: src.id(),
            graph: graph.group_hash(last),
            scale: scale.to_bits(),
        };
        let need = crate::prewarp::source_box(plan, src.width, src.height);
        if let Some((k, roi, s)) = &self.prewarped
            && *k == key
            && roi.contains(&need)
        {
            return Ok(s.clone());
        }
        let req = Request {
            precision: src.precision,
            checkpoints: false,
            scale,
            ..Request::new(need)
        };
        let r = self.engine.render(&graph, src, &req)?;
        let bpp = src.precision.bytes_per_pixel();
        let bytes = (u64::from(src.width) * u64::from(src.height) * bpp).max(16);
        // The last result's buffer is written again (a denoise drag allocates nothing).
        let buf = match self.prewarped.take() {
            Some((_, _, s)) if s.buffer().size() == bytes => s.buffer().clone(),
            _ => self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop denoised source"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        };
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("develop denoised source"),
            });
        crate::prewarp::copy_into(&mut enc, r.buffer(), need, &buf, src.width, bpp);
        self.queue.submit([enc.finish()]);
        self.engine.recycle(r);
        let s = self
            .engine
            .source_from_buffer(buf, src.width, src.height, src.precision)?;
        self.prewarped = Some((key, need, s.clone()));
        Ok(s)
    }

    /// The source after the geometry warp (`src` itself when the warp moves nothing), cached
    /// while the geometry, orientation and source stay the same. The output keeps the
    /// source's resolution (pixels per sensor pixel).
    fn warped_source(&mut self, edit: &Edit, src: &Source) -> Result<Source, Error> {
        let mut ctx = self.context(src);
        ctx.rgb_ca = self.rgb_ca(edit);
        // The size needs no lens model (the full plan is built only when the key changes).
        let mut light = ctx.clone();
        light.lens = None;
        light.lensfun = None;
        light.list3.clear();
        let k = f64::from(src.width) / ctx.frame[2];
        let size = Plan::new(edit, &light, self.crop_on).output_size(k);
        let geometry = [
            ModuleKind::LENS,
            ModuleKind::PERSPECTIVE,
            ModuleKind::CROP_ORIENTATION,
        ]
        .iter()
        .map(|&kind| {
            edit.modules.iter().find(|m| m.kind == kind).map(|m| {
                if !m.enabled {
                    return Params::Other(kind, Default::default());
                }
                match &m.params {
                    // Without the crop, moving it changes nothing here.
                    Params::CropOrientation(p) if !self.crop_on => {
                        let mut p = p.clone();
                        p.crop = [0.0, 0.0, 1.0, 1.0];
                        p.aspect = String::new();
                        Params::CropOrientation(p)
                    }
                    p => p.clone(),
                }
            })
        })
        .collect();
        let mut geometry: Vec<Option<Params>> = geometry;
        if let Some(ca) = ctx.rgb_ca {
            let f = crate::model::Fields::new()
                .with("red", crate::model::Value::Float(ca[0]))
                .with("blue", crate::model::Value::Float(ca[1]));
            geometry.push(Some(Params::Other(ModuleKind::CHROMATIC_ABERRATION, f)));
        }
        let key = WarpKey {
            source: src.id(),
            geometry,
            base: self.base,
            crop_on: self.crop_on,
            size,
        };
        if let Some((k, s)) = &self.warped
            && *k == key
        {
            return Ok(s.clone());
        }
        let plan = Plan::new(edit, &ctx, self.crop_on);
        if plan.is_identity() {
            return Ok(src.clone());
        }
        // The previous warp's buffer is written again (a straighten drag allocates nothing).
        if let Some((_, old)) = self.warped.take() {
            self.engine.forget();
            self.warper.give_back(old.buffer().clone());
        }
        let buf = match self.oversampled.as_ref().filter(|(id, _)| *id == src.id()) {
            // The larger source warped at its own scale, then area-reduced to `size`.
            Some((_, big)) => {
                let k2 = f64::from(big.width) / ctx.frame[2];
                let (bw, bh) = Plan::new(edit, &light, self.crop_on).output_size(k2);
                let big = big.clone();
                let tmp = self.warper.run(
                    &plan,
                    big.buffer(),
                    big.width,
                    big.height,
                    big.precision,
                    bw,
                    bh,
                    Precision::F32,
                )?;
                let r = self.reduce(&tmp, bw, bh, Precision::F32, size.0, size.1);
                self.warper.give_back(tmp);
                r?
            }
            None => self.warper.run(
                &plan,
                src.buffer(),
                src.width,
                src.height,
                src.precision,
                size.0,
                size.1,
                src.precision,
            )?,
        };
        let s = self
            .engine
            .source_from_buffer(buf, size.0, size.1, src.precision)?;
        self.engine.forget();
        self.warped = Some((key, s.clone()));
        Ok(s)
    }

    /// Area-weighted reduction of a packed `w`×`h` buffer to `dw`×`dh` (f16).
    fn reduce(
        &mut self,
        src: &wgpu::Buffer,
        w: u32,
        h: u32,
        prec: Precision,
        dw: u32,
        dh: u32,
    ) -> Result<wgpu::Buffer, Error> {
        let dst = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop preview source"),
            size: (u64::from(dw) * u64::from(dh) * 8).max(16),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let p = u32::from(prec == Precision::F16);
        self.dispatch(
            &self.aux.reduce,
            [w, h, dw, dh, p, 0, 0, 0],
            &dst,
            src,
            None,
        )?;
        Ok(dst)
    }

    /// Records and submits one helper pass over `u[2]×u[3]` threads.
    fn dispatch(
        &self,
        pipeline: &wgpu::ComputePipeline,
        u: [u32; 8],
        dst: &wgpu::Buffer,
        src: &wgpu::Buffer,
        then: Option<&dyn Fn(&mut wgpu::CommandEncoder)>,
    ) -> Result<(), Error> {
        let device = &self.device;
        let ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop preview params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&ub, 0, bytemuck::cast_slice(&u));
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("develop preview"),
            layout: &self.aux.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ub.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: dst.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: src.as_entire_binding(),
                },
            ],
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("develop preview"),
        });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("develop preview"),
                timestamp_writes: None,
            });
            cp.set_pipeline(pipeline);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(u[2].div_ceil(WG), u[3].div_ceil(WG), 1);
        }
        if let Some(f) = then {
            f(&mut enc);
        }
        self.busy.store(true, Ordering::Release);
        self.queue.submit([enc.finish()]);
        let busy = self.busy.clone();
        self.queue
            .on_submitted_work_done(move || busy.store(false, Ordering::Release));
        Ok(())
    }

    /// The colour (linear Rec.2020) where module `before` of `edit` sees the point at
    /// fraction `(u, v)` of the picture as rendered (after the geometry warp: upright and
    /// cropped): that module and every later one left out, the mean of a 5×5 patch. Reads
    /// back from the GPU: one call per pick (point colour).
    pub fn sample(
        &mut self,
        edit: &Edit,
        before: ModuleKind,
        u: f32,
        v: f32,
    ) -> Result<[f32; 3], Error> {
        let unwarped = self
            .source
            .clone()
            .ok_or(Error::BadRequest("prepare the source first"))?;
        let reg = crate::model::Registry::builtin();
        let cut = reg.rank(before);
        let mut e = edit.clone();
        for m in &mut e.modules {
            if reg.rank(m.kind) >= cut {
                m.enabled = false;
            }
        }
        let (src, after) = self.picture(edit, &e, &unwarped, None)?;
        let e = after.unwrap_or(e);
        let at = |f: f32, n: u32| ((f.clamp(0.0, 1.0) * n as f32) as u32).min(n - 1);
        let (x, y) = (at(u, src.width), at(v, src.height));
        let scale = self.view_scale(&unwarped);
        let graph = self.graph(&e, &unwarped, &src, None, None, scale, None)?;
        let (x, y) = (x.min(src.width - 1), y.min(src.height - 1));
        let (x0, y0) = (x.saturating_sub(2), y.saturating_sub(2));
        let roi = Roi::new(
            x0,
            y0,
            (x + 3).min(src.width) - x0,
            (y + 3).min(src.height) - y0,
        );
        let req = Request {
            precision: Precision::F32,
            scale: self.view_scale(&unwarped),
            checkpoints: false,
            ..Request::new(roi)
        };
        let r = self.engine.render(&graph, &src, &req)?;
        let px = self.engine.read(&r);
        self.engine.recycle(r);
        let px = px?;
        let n = (px.len() / 4).max(1) as f32;
        let mut sum = [0f32; 3];
        for p in px.chunks_exact(4) {
            for k in 0..3 {
                sum[k] += p[k];
            }
        }
        Ok(sum.map(|s| s / n))
    }

    /// The whole prepared picture (after the geometry warp) as module `before` of `edit`
    /// sees it: that module and every later one left out, linear Rec.2020 RGBA read back
    /// from the GPU (match to reference, at a small prepared size).
    pub fn scene_pixels(
        &mut self,
        edit: &Edit,
        before: ModuleKind,
    ) -> Result<(u32, u32, Vec<f32>), Error> {
        let unwarped = self
            .source
            .clone()
            .ok_or(Error::BadRequest("prepare the source first"))?;
        let reg = crate::model::Registry::builtin();
        let cut = reg.rank(before);
        let mut e = edit.clone();
        for m in &mut e.modules {
            if reg.rank(m.kind) >= cut {
                m.enabled = false;
            }
        }
        let (src, after) = self.picture(edit, &e, &unwarped, None)?;
        let e = after.unwrap_or(e);
        let scale = self.view_scale(&unwarped);
        let graph = self.graph(&e, &unwarped, &src, None, None, scale, None)?;
        let req = Request {
            precision: Precision::F32,
            scale,
            checkpoints: false,
            ..Request::new(Roi::full(src.width, src.height))
        };
        let r = self.engine.render(&graph, &src, &req)?;
        let px = self.engine.read(&r);
        self.engine.recycle(r);
        Ok((src.width, src.height, px?))
    }

    /// The line segments automatic upright works from
    /// ([`crate::ops::geometry::upright::auto`]): LSD on the lens-corrected, uncropped and
    /// unstraightened frame as the modules before exposure leave it, at most
    /// [`crate::ops::geometry::upright::PICTURE_PIXELS`]. Segments are in the oriented
    /// frame (square pixels, centred); with the plan of that picture.
    pub fn upright_lines(&mut self, edit: &Edit) -> Result<UprightLines, Error> {
        let mut e = edit.clone();
        for m in &mut e.modules {
            if m.kind == ModuleKind::PERSPECTIVE {
                m.enabled = false;
            }
            if m.kind == ModuleKind::CROP_ORIENTATION {
                let mut p =
                    crate::model::params::CropOrientation::from_fields(&m.params.to_fields());
                p.angle_deg = 0.0;
                p.crop = [0.0, 0.0, 1.0, 1.0];
                m.params = Params::CropOrientation(p);
            }
        }
        let crop_on = std::mem::replace(&mut self.crop_on, false);
        let px = self.scene_pixels(&e, ModuleKind::EXPOSURE);
        let plan = self.plan(&e);
        self.crop_on = crop_on;
        let (w, h, px) = px?;
        let oriented = plan
            .as_ref()
            .map_or((f64::from(w), f64::from(h)), |p| p.oriented);
        let lines =
            crate::ops::geometry::upright::lines_of_picture(&px, w as usize, h as usize, oriented);
        Ok(UprightLines {
            lines,
            oriented,
            straighten: crate::ops::geometry::crop_params(edit).angle_deg,
            focal35: self.focal35,
        })
    }

    /// The tone equalizer's mask percentiles for `mask_auto` (5th and 95th, EV) on the
    /// picture it sees, for zone picks ([`ToneEq::zone_of`]); `None` when it has no automatic
    /// mask. Reads the whole picture back once.
    ///
    /// [`ToneEq::zone_of`]: crate::ops::tone::equalizer::ToneEq::zone_of
    pub fn tone_eq_percentiles(&mut self, edit: &Edit) -> Result<Option<[f32; 2]>, Error> {
        use crate::ops::tone::equalizer::ToneEq;
        let te = edit
            .modules
            .iter()
            .find(|m| m.kind == ModuleKind::TONE_EQUALIZER && m.enabled)
            .map(|m| ToneEq::from_fields(&m.params.to_fields()))
            .filter(|t| t.mask_auto);
        let Some(te) = te else { return Ok(None) };
        let (w, h, px) = self.scene_pixels(edit, ModuleKind::TONE_EQUALIZER)?;
        Ok(te.mask_percentiles(w, h, &px))
    }

    /// The picture AI masks are computed on ([`crate::mask::ai`]): the whole frame
    /// upright (the file's orientation; no crop, straighten, turns or lens correction), as
    /// the modules before exposure leave it, at the prepared size. With the plan mapping
    /// it onto the source frame (None: it is the frame) and the frame's size in sensor
    /// pixels.
    pub fn ai_picture(&mut self, edit: &Edit) -> Result<AiPicture, Error> {
        let mut e = edit.clone();
        for m in &mut e.modules {
            if matches!(
                m.kind,
                ModuleKind::CROP_ORIENTATION | ModuleKind::LENS | ModuleKind::PERSPECTIVE
            ) {
                m.enabled = false;
            }
        }
        let (w, h, rgba) = self.scene_pixels(&e, ModuleKind::EXPOSURE)?;
        let plan = self.plan(&e);
        let frame = match &plan {
            Some(p) => (p.frame[2], p.frame[3]),
            None => {
                let (sw, sh) = self.source_size().unwrap_or((w, h));
                (f64::from(sw), f64::from(sh))
            }
        };
        let plan = plan.filter(|p| !p.is_identity());
        Ok((crate::mask::ai::Picture { w, h, rgba }, plan, frame))
    }

    /// The noise model of the prepared source (what AI denoise's Anscombe encoding
    /// uses), when known.
    pub fn noise_model(&mut self) -> Option<crate::ops::detail::NoiseModel> {
        self.source_noise()
    }

    /// Why the raw front end's profile file (`InputProfile::File`) is not used by the
    /// source shown, when it was asked for and is not (the camera matrix stands in).
    pub fn input_profile_problem(&self) -> Option<&crate::ops::raw::profile_file::Problem> {
        self.input_profile_problem.as_ref()
    }

    /// Sets the photo's identity ([`crate::mask::ai::photo_id`]) after [`Developer::set_input`]:
    /// it keys denoise's AI result ([`crate::ops::detail::ai`]).
    pub fn set_photo(&mut self, photo: Option<[u8; 32]>) {
        self.detail.photo = photo;
    }

    /// The view scale (developed size over the sensor's): neighbourhood ops work on the
    /// same source-pixel scales at every size.
    fn view_scale(&self, src: &Source) -> f32 {
        self.sensor.as_ref().map_or(1.0, |s| {
            let c = s.sensor().crop;
            let bin = self.sensor_bin.max(1.0);
            (src.width.max(src.height) as f32 / (c.w.max(c.h).max(1) as f32 * bin)).clamp(1e-3, 1.0)
        })
    }

    /// Renders `edit` (with `focus` the module being edited) into `out`'s space.
    pub fn render(
        &mut self,
        edit: &Edit,
        focus: Option<(ModuleKind, u16)>,
        out: &OutputSpace,
    ) -> Result<Rendered, Error> {
        let unwarped = self
            .source
            .clone()
            .ok_or(Error::BadRequest("prepare the source first"))?;
        let denoise = edit
            .modules
            .iter()
            .any(|m| m.kind == ModuleKind::DENOISE && m.enabled);
        if denoise {
            self.detail.noise = self.source_noise();
        }
        let overlay = self.mask_overlay;
        self.stats.half_draft = false;
        self.stats.draft_divisor = 1;
        self.dragging = focus.is_some();
        let started = std::time::Instant::now();
        let first = std::mem::take(&mut self.fresh) && self.draft_first && focus.is_none();
        let divisor = match (focus, self.drag_level) {
            // "Preview while dragging" sizes the drafts of a slider: every module but those
            // with drafts of their own, and not while a mask is shown.
            (Some(f), Some(level)) => {
                if overlay.is_none() && !OWN_DRAFTS.contains(&f.0) {
                    fit_divisor(level.divisor(), unwarped.width, unwarped.height)
                } else {
                    1
                }
            }
            // The first picture as if every module were dragged (denoise after the warp).
            _ => {
                let draft = match focus {
                    Some(f) => halves(edit, f),
                    None => first && halves(edit, (ModuleKind::DENOISE, 0)),
                };
                let large =
                    u64::from(unwarped.width) * u64::from(unwarped.height) >= HALF_DRAFT_PIXELS;
                if overlay.is_none() && self.half_drafts && draft && large {
                    2
                } else {
                    1
                }
            }
        };
        if divisor > 1 {
            let r = self.render_half(
                edit,
                focus.or(Some((ModuleKind::DENOISE, 0))),
                out,
                &unwarped,
                divisor,
            );
            self.draft_shown = focus.is_none() && r.is_ok();
            self.time_frame(started);
            return r;
        }
        let with = with_image(edit, &self.detail);
        let edit = with.as_ref().unwrap_or(edit);
        let (src, after) = self.picture(edit, edit, &unwarped, overlay)?;
        let edit = after.as_ref().unwrap_or(edit);
        // The warp keeps the unwarped source's pixels per sensor pixel.
        let scale = self.view_scale(&unwarped);
        let graph = self.graph(edit, &unwarped, &src, focus, Some(out), scale, overlay)?;
        let req = Request {
            profile: self.profile && self.engine.has_timestamps(),
            scale,
            tiling: self.tile_side.map_or(
                crate::graph::tile::Tiling::Auto,
                crate::graph::tile::Tiling::Side,
            ),
            // A refined picture is kept: undo and redo find it.
            keep_result: self.keep_results && focus.is_none(),
            // And so is every group's output on the way, so a change to a later
            // module resumes right before it (whole pictures only).
            checkpoint_every: self.checkpoint_every && focus.is_none() && self.tile_side.is_none(),
            ..Request::new(Roi::full(src.width, src.height))
        };
        // The first refined picture after a change without sharpening; the whole
        // one follows (`refine_due`), resuming at sharpening from this one's checkpoints.
        let mut deferred = false;
        let graph = if focus.is_none()
            && overlay.is_none()
            && self.defer_sharpen
            && req.checkpoint_every
            && sharpens(edit)
            && let Some(full) = graph
                .groups
                .len()
                .checked_sub(1)
                .map(|g| graph.group_hash(g))
        {
            // Only when the change is before sharpening (else the whole picture resumes at
            // sharpening or later anyway), and not for a kept result (undo, redo).
            let sharpen_at = graph
                .groups
                .iter()
                .position(|g| {
                    graph.nodes[g.nodes.clone()]
                        .iter()
                        .any(|n| n.id.0 == ModuleKind::SHARPEN)
                })
                .unwrap_or(0);
            if self.sharpen_deferred == Some(full)
                || self.engine.has_result(&graph, &src, &req)
                || self.engine.resumes_at(&graph, &src, &req) >= sharpen_at
            {
                self.sharpen_deferred = None;
                graph
            } else {
                let mut without = edit.clone();
                for m in &mut without.modules {
                    if m.kind == ModuleKind::SHARPEN {
                        m.enabled = false;
                    }
                }
                let g = self.graph(&without, &unwarped, &src, None, Some(out), scale, None)?;
                self.sharpen_deferred = Some(full);
                deferred = true;
                g
            }
        } else {
            graph
        };
        self.stats.sharpen_deferred = deferred;
        self.busy.store(true, Ordering::Release);
        let r = self.engine.render(&graph, &src, &req);
        let busy = self.busy.clone();
        self.queue
            .on_submitted_work_done(move || busy.store(false, Ordering::Release));
        let r = r?;
        let r = match overlay {
            Some((target, _)) if graph.tint_apart => {
                self.tint_apart(edit, &unwarped, &src, focus, out, scale, target, &req, r)?
            }
            _ => r,
        };
        self.stats.render_gpu_ms = r.stats.gpu_total_ms();
        self.stats.pipelines_created = self.engine.counters().pipelines;
        if deferred {
            self.draft_shown = true;
        }
        self.time_frame(started);
        Ok(r)
    }

    /// Whether a drag of `focus` on `edit` is drafted at half scale without a
    /// [`Developer::drag_level`] (where "Preview while dragging" starts when it knows
    /// nothing better): a large picture with an op that reads neighbours at or after the
    /// focus.
    pub fn drag_default(&self, edit: &Edit, focus: (ModuleKind, u16)) -> crate::drag::Level {
        let large = self
            .source
            .as_ref()
            .is_some_and(|s| u64::from(s.width) * u64::from(s.height) >= HALF_DRAFT_PIXELS);
        if self.half_drafts && large && halves(edit, focus) {
            crate::drag::Level::Half
        } else {
            crate::drag::Level::Full
        }
    }

    /// Measures every render from the call to the GPU being done, on a thread that waits
    /// for the device ([`Developer::take_frame_ms`]); off by default (the app turns it on
    /// for "Preview while dragging").
    pub fn time_frames(&mut self, on: bool) {
        if !on {
            self.clock = None;
        } else if self.clock.is_none() {
            self.clock = FrameClock::new(&self.device);
        }
    }

    /// How long the last render took, once the GPU is done with it (taken once).
    pub fn take_frame_ms(&mut self) -> Option<f64> {
        let c = self.clock.as_mut()?;
        let mut done = c
            .done
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match *done {
            Some((seq, ms)) if seq == c.seq => {
                *done = None;
                Some(ms)
            }
            _ => None,
        }
    }

    fn time_frame(&mut self, started: std::time::Instant) {
        if let Some(c) = self.clock.as_mut() {
            c.seq += 1;
            let _ = c.tx.send((c.seq, started));
        }
    }

    /// A draft at half view scale: the warped picture reduced to half size (kept while the
    /// picture stays), `edit`'s graph with denoise after the warp on it, scaled up to the
    /// picture's size.
    fn render_half(
        &mut self,
        edit: &Edit,
        focus: Option<(ModuleKind, u16)>,
        out: &OutputSpace,
        unwarped: &Source,
        divisor: u32,
    ) -> Result<Rendered, Error> {
        // Denoise dragged (or what precedes it): it runs after the warp at half scale.
        // Otherwise the picture is the refined one's (denoised before the warp, kept).
        let reg = crate::model::Registry::builtin();
        let early = focus.is_some_and(|(k, _)| reg.rank(k) <= reg.rank(ModuleKind::DENOISE));
        let full = with_image(edit, &self.detail);
        let (src, after) = if early {
            (self.warped_source(edit, unwarped)?, None)
        } else {
            let e = full.as_ref().unwrap_or(edit);
            self.picture(e, e, unwarped, None)?
        };
        let small = match &self.half {
            Some((id, d, s)) if *id == src.id() && *d == divisor => s.clone(),
            _ => {
                let (hw, hh) = (src.width.div_ceil(divisor), src.height.div_ceil(divisor));
                let buf =
                    self.reduce(src.buffer(), src.width, src.height, src.precision, hw, hh)?;
                let s = self
                    .engine
                    .source_from_buffer(buf, hw, hh, Precision::F16)?;
                self.half = Some((src.id(), divisor, s.clone()));
                s
            }
        };
        let k = small.width as f32 / src.width as f32;
        // Averaging 2×2 pixels quarters the noise variance.
        let mut detail = self.detail;
        let area = f64::from(small.width) * f64::from(small.height)
            / (f64::from(src.width) * f64::from(src.height));
        detail.noise = detail.noise.map(|n| n.scaled(area as f32));
        let with = with_image(edit, &detail);
        let edit = match &after {
            Some(a) => a,
            None => with.as_ref().unwrap_or(edit),
        };
        let scale = self.view_scale(unwarped) * k;
        let graph = self.graph(edit, unwarped, &small, focus, Some(out), scale, None)?;
        let req = Request {
            profile: self.profile && self.engine.has_timestamps(),
            scale,
            ..Request::new(Roi::full(small.width, small.height))
        };
        self.busy.store(true, Ordering::Release);
        // The statistics on a grid of half (a quarter of) the side too.
        self.engine
            .set_stats_side(crate::graph::STATS_SIDE / divisor);
        let r = self.engine.render(&graph, &small, &req);
        self.engine.set_stats_side(crate::graph::STATS_SIDE);
        let busy = self.busy.clone();
        let r = r.and_then(|r| {
            let mut big = self
                .engine
                .blank(Roi::full(src.width, src.height), r.precision);
            let p = u32::from(r.precision == Precision::F16);
            let res = self.dispatch(
                &self.aux.upscale,
                [small.width, small.height, src.width, src.height, p, 0, 0, 0],
                big.buffer(),
                r.buffer(),
                None,
            );
            let stats = r.stats.clone();
            self.engine.recycle(r);
            match res {
                Ok(()) => {
                    big.stats = stats;
                    Ok(big)
                }
                Err(e) => {
                    self.engine.recycle(big);
                    Err(e)
                }
            }
        });
        self.queue
            .on_submitted_work_done(move || busy.store(false, Ordering::Release));
        let r = r?;
        self.stats.half_draft = true;
        self.stats.draft_divisor = divisor;
        self.stats.render_gpu_ms = r.stats.gpu_total_ms();
        self.stats.pipelines_created = self.engine.counters().pipelines;
        Ok(r)
    }

    /// The tint overlay of `target` drawn over `r` (the picture without it) from a grey
    /// overlay render: a node after the module overwrites the alpha the mask travels in.
    #[allow(clippy::too_many_arguments)]
    fn tint_apart(
        &mut self,
        edit: &Edit,
        unwarped: &Source,
        src: &Source,
        focus: Option<(ModuleKind, u16)>,
        out: &OutputSpace,
        scale: f32,
        target: (ModuleKind, u16),
        req: &Request,
        r: Rendered,
    ) -> Result<Rendered, Error> {
        let grey = self.graph(
            edit,
            unwarped,
            src,
            focus,
            Some(out),
            scale,
            Some((target, Overlay::Grey)),
        );
        let m = match grey.and_then(|g| self.engine.render(&g, src, req)) {
            Ok(m) => m,
            Err(e) => {
                self.engine.recycle(r);
                return Err(e);
            }
        };
        let (w, h) = (r.roi.w, r.roi.h);
        let p = u32::from(r.precision == Precision::F16);
        let res = self.dispatch(
            &self.aux.tint,
            [w, h, w, h, p, 0, 0, 0],
            r.buffer(),
            m.buffer(),
            None,
        );
        self.engine.recycle(m);
        match res {
            Ok(()) => Ok(r),
            Err(e) => {
                self.engine.recycle(r);
                Err(e)
            }
        }
    }

    /// Writes `r`, turned by `orient`, into `texture` (the turned size, usage `COPY_DST`),
    /// and gives the render's buffer back. An `Rgba8UnormSrgb` texture gets sRGB-encoded
    /// values clipped to 0..1; an `Rgba16Float` one linear values as they are (HDR: a render
    /// whose output peak is above 1 keeps its highlights).
    pub fn present(
        &mut self,
        r: Rendered,
        orient: Orient,
        texture: &wgpu::Texture,
    ) -> Result<(), Error> {
        let (sw, sh) = (r.roi.w, r.roi.h);
        let (ow, oh) = orient.size(sw, sh);
        if texture.width() != ow || texture.height() != oh {
            self.engine.recycle(r);
            return Err(Error::BadRequest(
                "the texture does not have the turned size",
            ));
        }
        let hdr = texture.format() == wgpu::TextureFormat::Rgba16Float;
        let texel: u32 = if hdr { 8 } else { 4 };
        let stride = ow.next_multiple_of(64);
        let bytes = u64::from(stride) * u64::from(oh) * u64::from(texel);
        let staging = self.staging_buffer(bytes);
        let flags = u32::from(orient.transpose)
            | (u32::from(orient.flip_x) << 1)
            | (u32::from(orient.flip_y) << 2);
        let p = u32::from(r.precision == Precision::F16);
        let copy = |enc: &mut wgpu::CommandEncoder| {
            enc.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride * texel),
                        rows_per_image: Some(oh),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: ow,
                    height: oh,
                    depth_or_array_layers: 1,
                },
            );
        };
        let pipeline = if hdr {
            &self.aux.present_hdr
        } else {
            &self.aux.present
        };
        let res = self.dispatch(
            pipeline,
            [sw, sh, ow, oh, p, flags, stride, 0],
            &staging,
            r.buffer(),
            Some(&copy),
        );
        self.engine.recycle(r);
        res
    }

    /// `r` turned by `orient` as sRGB-encoded RGBA rows (the software-adapter path, whose
    /// device cannot write the UI's textures). Waits for the GPU.
    pub fn read_rgba8(
        &mut self,
        r: Rendered,
        orient: Orient,
    ) -> Result<(u32, u32, Vec<u8>), Error> {
        let (sw, sh) = (r.roi.w, r.roi.h);
        let (ow, oh) = orient.size(sw, sh);
        let stride = ow.next_multiple_of(64);
        let bytes = u64::from(stride) * u64::from(oh) * 4;
        let staging = self.staging_buffer(bytes);
        let flags = u32::from(orient.transpose)
            | (u32::from(orient.flip_x) << 1)
            | (u32::from(orient.flip_y) << 2);
        let p = u32::from(r.precision == Precision::F16);
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop preview read"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let copy = |enc: &mut wgpu::CommandEncoder| {
            enc.copy_buffer_to_buffer(&staging, 0, &read, 0, bytes);
        };
        let res = self.dispatch(
            &self.aux.present,
            [sw, sh, ow, oh, p, flags, stride, 0],
            &staging,
            r.buffer(),
            Some(&copy),
        );
        self.engine.recycle(r);
        res?;
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r.is_ok());
        });
        crate::gpu::poll(&self.device, wgpu::PollType::wait_indefinitely())?;
        if rx.recv_timeout(std::time::Duration::from_secs(30)) != Ok(true) {
            return Err(Error::Gpu("reading the preview back failed".into()));
        }
        let mut out = Vec::with_capacity(ow as usize * oh as usize * 4);
        {
            let view = read
                .slice(..)
                .get_mapped_range()
                .map_err(|e| Error::Gpu(format!("{e:?}")))?;
            for row in view.chunks_exact(stride as usize * 4) {
                out.extend_from_slice(&row[..ow as usize * 4]);
            }
        }
        read.unmap();
        Ok((ow, oh, out))
    }

    /// Soft-proofs `r` in place through `lut`: the picture becomes the proofed
    /// display colour, alpha the proof gamut ΔE. Call before [`Developer::scopes`] and
    /// [`Developer::clipping`] so they see what is shown.
    pub fn proof(&mut self, r: &Rendered, lut: &ProofLut) {
        let (device, queue) = (&self.device, &self.queue);
        let p = self
            .proofing
            .get_or_insert_with(|| ProofPass::new(device, queue));
        p.run(lut, r.buffer(), r.roi.w, r.roi.h, r.precision);
    }

    /// Starts the scopes of `r` (values up to `peak`); [`Developer::take_scopes`] has them
    /// once read back.
    pub fn scopes(&mut self, r: &Rendered, peak: f32) {
        let (device, queue) = (&self.device, &self.queue);
        let s = self
            .scopes
            .get_or_insert_with(|| ScopeEngine::new(device, queue));
        s.run(r.buffer(), r.roi.w, r.roi.h, r.precision, peak);
    }

    /// The newest scopes once read back (never waits), and whether a pass was skipped so
    /// the picture should render again to catch up.
    pub fn take_scopes(&mut self) -> (Option<Bins>, bool) {
        match self.scopes.as_mut() {
            Some(s) => {
                let b = s.take();
                (b, s.stale && !s.pending())
            }
            None => (None, false),
        }
    }

    /// Whether scopes are on their way back.
    pub fn scopes_pending(&self) -> bool {
        self.scopes.as_ref().is_some_and(ScopeEngine::pending)
    }

    /// Waits for the scopes (tests, timing).
    pub fn wait_scopes(&mut self) -> Option<Bins> {
        self.scopes.as_mut().and_then(ScopeEngine::wait)
    }

    /// Writes the clipping overlay of `r` (the render of `edit` into `out`, proofed when
    /// `proofed`) into `texture` (`Rgba8Unorm`, the picture's size): renders the scene
    /// before the look, and the display-referred picture before the output transform, when
    /// the overlays asked for need them.
    pub fn clipping(
        &mut self,
        edit: &Edit,
        r: &Rendered,
        flags: ClipFlags,
        out: &OutputSpace,
        proofed: bool,
        texture: &wgpu::Texture,
    ) -> Result<(), Error> {
        let reg = crate::model::Registry::builtin();
        let unwarped = self
            .source
            .clone()
            .ok_or(Error::BadRequest("prepare the source first"))?;
        let with = with_image(edit, &self.detail);
        let edit = with.as_ref().unwrap_or(edit);
        let (src, after) = self.picture(edit, edit, &unwarped, None)?;
        let edit = after.as_ref().unwrap_or(edit);
        let scale = self.view_scale(&unwarped);
        let req = Request {
            scale,
            ..Request::new(Roi::full(src.width, src.height))
        };
        let scene = if flags.scene {
            let cut = reg.rank(ModuleKind::VIEW_TRANSFORM);
            let mut e = edit.clone();
            for m in &mut e.modules {
                if reg.rank(m.kind) >= cut {
                    m.enabled = false;
                }
            }
            let g = self.graph(&e, &unwarped, &src, None, None, scale, None)?;
            Some(self.engine.render(&g, &src, &req)?)
        } else {
            None
        };
        let disp = if flags.gamut && !proofed {
            let g = self.graph(edit, &unwarped, &src, None, None, scale, None)?;
            Some(self.engine.render(&g, &src, &req)?)
        } else {
            None
        };
        let (scene_white, scene_black) = crate::clip::scene_limits(edit);
        let p = ClipParams {
            scene_white,
            scene_black,
            peak: out.peak,
            proof_gamut: proofed,
            to_output: out.from_rec2020,
        };
        let (device, queue) = (&self.device, &self.queue);
        let pass = self
            .clipping
            .get_or_insert_with(|| ClipPass::new(device, queue));
        let fits = scene.as_ref().is_none_or(|s| s.roi == r.roi)
            && disp.as_ref().is_none_or(|d| d.roi == r.roi);
        if fits {
            pass.run(
                (r.buffer(), r.precision),
                scene.as_ref().map(|s| (s.buffer(), s.precision)),
                disp.as_ref().map(|d| (d.buffer(), d.precision)),
                r.roi.w,
                r.roi.h,
                &p,
                flags,
                Some(texture),
            );
        }
        for x in [scene, disp].into_iter().flatten() {
            self.engine.recycle(x);
        }
        Ok(())
    }

    /// Pipelines made by the engine and the aids so far (an edit change should make none).
    pub fn pipelines(&self) -> u64 {
        self.engine.counters().pipelines
            + u64::from(self.scopes.is_some())
            + u64::from(self.proofing.is_some())
            + u64::from(self.clipping.is_some())
    }

    fn staging_buffer(&mut self, bytes: u64) -> wgpu::Buffer {
        if let Some(b) = &self.staging
            && b.size() >= bytes
        {
            return b.clone();
        }
        let b = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop preview rows"),
            size: bytes.max(256),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.staging = Some(b.clone());
        b
    }

    /// Waits for the GPU (tests and timing).
    pub fn wait(&self) -> Result<(), Error> {
        self.engine.wait()?;
        self.busy.store(false, Ordering::Release);
        Ok(())
    }

    /// Reads a render back as f32 RGBA (tests).
    pub fn read(&self, r: &Rendered) -> Result<Vec<f32>, Error> {
        self.engine.read(r)
    }

    /// Gives a render's buffer back without presenting it.
    pub fn recycle(&mut self, r: Rendered) {
        self.engine.recycle(r);
    }
}

/// `rgba` box-reduced by a whole factor so its long side is at most `long`.
fn reduce_rgba(w: u32, h: u32, rgba: Vec<f32>, long: u32) -> (u32, u32, Vec<f32>) {
    let f = w.max(h).div_ceil(long).max(1);
    if f == 1 {
        return (w, h, rgba);
    }
    let (rw, rh) = (w / f, h / f);
    let mut out = vec![0f32; (rw * rh * 4) as usize];
    let inv = 1.0 / (f * f) as f32;
    for y in 0..rh {
        for x in 0..rw {
            let o = ((y * rw + x) * 4) as usize;
            for yy in y * f..(y + 1) * f {
                for xx in x * f..(x + 1) * f {
                    let i = ((yy * w + xx) * 4) as usize;
                    for k in 0..4 {
                        out[o + k] += rgba[i + k] * inv;
                    }
                }
            }
        }
    }
    (rw, rh, out)
}

/// Reads a raw file's sensor data for [`Input::Raw`] (a plain-language error otherwise).
pub fn decode_raw(data: &[u8]) -> Result<Input, String> {
    let f = specular_raw::parse(data).map_err(|e| e.to_string())?;
    if !f.can_decode_sensor() {
        return Err("this raw's sensor data cannot be developed yet".into());
    }
    f.decode_sensor(data, &specular_raw::DecodeOptions::default())
        // Float DNGs (HDR merges) at their reference brightness.
        .map(|s| Input::Raw(Box::new(crate::merge::apply_baseline(s))))
        .map_err(|e| e.to_string())
}

/// Whether a draft of `edit` with `focus` dragged renders at half view scale: the module
/// has no draft of its own, and it or an enabled module after it reads neighbours.
/// The smallest picture a reduced draft is drawn at (long side, pixels).
pub const MIN_DRAFT_LONG: u32 = 256;

/// `divisor` lowered (by halves) until a `w`×`h` picture reduced by it keeps a long side of
/// at least [`MIN_DRAFT_LONG`].
fn fit_divisor(divisor: u32, w: u32, h: u32) -> u32 {
    let mut d = divisor.max(1);
    while d > 1 && w.max(h) / d < MIN_DRAFT_LONG {
        d /= 2;
    }
    d
}

/// Waits for the device on a thread of its own after each render it is told of, and keeps
/// the time from the render call to the GPU being done ([`Developer::time_frames`]): the
/// caller's own polls come once a display frame and would round every time up to one.
struct FrameClock {
    tx: std::sync::mpsc::Sender<(u64, std::time::Instant)>,
    /// The last render told of.
    seq: u64,
    /// A render's number and milliseconds, once the GPU is done with it.
    done: Arc<std::sync::Mutex<Option<(u64, f64)>>>,
}

impl FrameClock {
    fn new(device: &wgpu::Device) -> Option<FrameClock> {
        let (tx, rx) = std::sync::mpsc::channel::<(u64, std::time::Instant)>();
        let done = Arc::new(std::sync::Mutex::new(None));
        let (device, out) = (device.clone(), done.clone());
        specular_thread::builder("dv-frame-clock")
            .spawn(move || {
                // Ends when the developer (the sender) goes, or the device.
                while let Ok((seq, started)) = rx.recv() {
                    if crate::gpu::poll(&device, wgpu::PollType::wait_indefinitely()).is_err() {
                        return;
                    }
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    *out.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((seq, ms));
                }
            })
            .ok()?;
        Some(FrameClock { tx, seq: 0, done })
    }
}

fn halves(edit: &Edit, focus: (ModuleKind, u16)) -> bool {
    if OWN_DRAFTS.contains(&focus.0) {
        return false;
    }
    let reg = crate::model::Registry::builtin();
    let ops = OpRegistry::builtin();
    let at = reg.rank(focus.0);
    edit.modules.iter().any(|m| {
        m.enabled
            && reg.rank(m.kind) >= at
            && reg.get(m.kind).is_some_and(|s| {
                !matches!(
                    s.stage,
                    crate::model::Stage::Raw
                        | crate::model::Stage::Warp
                        | crate::model::Stage::Output
                )
            })
            && ops
                .get(m.kind, m.version)
                .is_some_and(|op| !op.kind().pointlike() && !op.identity(&m.params))
    })
}

/// Whether `edit` sharpens (an enabled sharpen module).
fn sharpens(edit: &Edit) -> bool {
    edit.modules
        .iter()
        .any(|m| m.kind == ModuleKind::SHARPEN && m.enabled)
}

/// Whether `edit`'s geometry warp resamples an image that is not a raw (lens distortion
/// or colour fringes, chromatic aberration, which other images correct in the warp, or a
/// straighten): its source is then sampled at up to twice the view.
pub fn rgb_warp_resamples(edit: &Edit) -> bool {
    let lens = crate::ops::geometry::lens_params(edit)
        .is_some_and(|l| l.distortion != 0.0 || l.tca_red != 0.0 || l.tca_blue != 0.0);
    lens || crate::ops::correct::chromatic_aberration(edit).is_some()
        || crate::ops::geometry::crop_params(edit).angle_deg != 0.0
        || persp_resamples(edit)
}

/// Whether `edit` has a perspective that moves pixels.
fn persp_resamples(edit: &Edit) -> bool {
    crate::ops::geometry::persp::persp_params(edit).is_some_and(|p| !p.is_neutral())
}

/// [`fit`] at `long / m`, times `m` (both sides whole multiples of `m`), when that is
/// smaller than `w × h`; otherwise [`fit`].
pub fn fit_multiple(w: u32, h: u32, long: u32, m: u32) -> (u32, u32) {
    let m = m.max(1);
    let (a, b) = fit(w, h, (long / m).max(1));
    if m == 1 || (a, b) == (w, h) || a * m > w || b * m > h {
        return fit(w, h, long);
    }
    (a * m, b * m)
}

/// `w`×`h` fitted inside a long side of `long` (never enlarged).
pub fn fit(w: u32, h: u32, long: u32) -> (u32, u32) {
    let m = w.max(h);
    if m <= long {
        return (w, h);
    }
    let k = f64::from(long) / f64::from(m);
    (
        ((f64::from(w) * k).round() as u32).max(1),
        ((f64::from(h) * k).round() as u32).max(1),
    )
}

/// 8-bit sRGB-encoded RGBA → linear Rec.2020 RGBA (alpha straight), area-reduced to a
/// long side of at most `long` (for [`Input::Rgb`]).
pub fn rgba8_to_input(w: u32, h: u32, rgba: &[u8], long: u32) -> Input {
    let lin: Vec<f32> = (0..256)
        .map(|i| {
            let c = i as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
        .collect();
    let m = crate::ops::m3_f32(&crate::colour::rec709_to_rec2020());
    let (dw, dh) = fit(w, h, long);
    let (sx, sy) = (w as f32 / dw as f32, h as f32 / dh as f32);
    let mut out = vec![0f32; dw as usize * dh as usize * 4];
    for oy in 0..dh {
        let y0 = oy as f32 * sy;
        let y1 = y0 + sy;
        for ox in 0..dw {
            let x0 = ox as f32 * sx;
            let x1 = x0 + sx;
            let mut acc = [0f32; 4];
            let mut ws = 0f32;
            for y in (y0.floor() as u32)..(y1.ceil() as u32).min(h) {
                let wy = (y as f32 + 1.0).min(y1) - (y as f32).max(y0);
                for x in (x0.floor() as u32)..(x1.ceil() as u32).min(w) {
                    let wx = (x as f32 + 1.0).min(x1) - (x as f32).max(x0);
                    let wgt = wx * wy;
                    if wgt <= 0.0 {
                        continue;
                    }
                    let i = (y as usize * w as usize + x as usize) * 4;
                    for c in 0..3 {
                        acc[c] += lin[usize::from(rgba[i + c])] * wgt;
                    }
                    acc[3] += f32::from(rgba[i + 3]) / 255.0 * wgt;
                    ws += wgt;
                }
            }
            let v = acc.map(|a| a / ws.max(1e-12));
            let o = (oy as usize * dw as usize + ox as usize) * 4;
            for i in 0..3 {
                out[o + i] = m[i * 3] * v[0] + m[i * 3 + 1] * v[1] + m[i * 3 + 2] * v[2];
            }
            out[o + 3] = v[3];
        }
    }
    Input::Rgb {
        width: dw,
        height: dh,
        rgba: out,
    }
}

#[cfg(test)]
mod adrag_tests;
#[cfg(test)]
mod aids_tests;
#[cfg(test)]
mod binned_tests;
#[cfg(test)]
mod draft_tests;
#[cfg(test)]
mod fused_tests;
#[cfg(test)]
mod prewarp_tests;
#[cfg(test)]
mod refine_tests;
#[cfg(test)]
mod tests;
