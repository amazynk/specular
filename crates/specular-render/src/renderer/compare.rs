//! Compare's GPU passes (DESIGN.md §6.3): the difference view (false colour, signed, or over
//! the image), the metrics of the visible region (MAE, MSE, PSNR, SSIM, ΔE2000) and the dual
//! pixel probe.
//!
//! - The difference view reads two renders of the same frame (the caller's, made with
//!   [`ImageRenderer::render`] over nothing: premultiplied linear light, the channel view
//!   applied) and writes the per-pixel error through the false-colour ramp
//!   ([`Comparer::draw_diff`]).
//! - The metrics render the visible part of the overlap of both images into two sample
//!   targets of at most [`METRIC_SIZE`]² (nearest-neighbour: one texel of the finest
//!   resident level per sample, never a blend), reduce the error in two compute passes
//!   (16 × 16 blocks into partials, then the partials into one total) and read back 16 bytes
//!   per pair. WGSL has no float atomics, hence the two passes. SSIM (`compare_ssim.wgsl`)
//!   and ΔE2000 (`compare_de.wgsl`, with a 256-bin histogram for the 95th percentile) reduce
//!   the same sample targets the same way into slots of their own.
//! - Each image has a [`Place`] in the frame both are compared in (top-left, centred, scaled
//!   to the first, or aligned by a similarity), so a pair's sample targets hold the same
//!   frame region of both.
//! - The probe copies one texel of each pane's render ([`Comparer::request_probe`]).
//!
//! The definitions (and the CPU references the tests check) are `specular_inspect::diff` and
//! `specular_inspect::quality`.
//! As with [`super::inspect::Inspector`], readbacks never stall a frame: [`Comparer::request`]
//! records, [`Comparer::after_submit`] maps, [`Comparer::poll`] picks the result up later.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bytemuck::{Pod, Zeroable};
use specular_core::Orientation;
use specular_gfx::BufferInit;
use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, wgpu};
use specular_inspect::quality::{DE_BINS, DeltaEStats, ssim_weights};
use specular_inspect::{ChannelView, ColourMap, DiffKind, DiffStats, DiffSums, MetricSpace};

use super::{GpuImageCache, ImageRenderer, RenderOptions};
use crate::view::{PxRect, View};

/// Largest side of the metric sample targets.
pub const METRIC_SIZE: u32 = 2048;
/// Image pairs one metrics request covers (compare shows up to four images: three pairs).
pub const MAX_PAIRS: usize = 3;
/// Totals per pair: the error sums, SSIM (sum, count) and ΔE (sum, max).
const SLOTS: usize = 3;
const TOTAL_BYTES: u64 = (MAX_PAIRS * SLOTS) as u64 * 16;
const HIST_BYTES: u64 = (MAX_PAIRS * DE_BINS) as u64 * 4;
const MAX_GROUPS: u64 = (METRIC_SIZE as u64 / 16) * (METRIC_SIZE as u64 / 16);
/// Panes one probe reads (A and B); texture copies want 256-byte rows.
const PROBE_TEXELS: usize = 2;
const PROBE_ROW: u64 = 256;

/// The diverging map of the signed difference (Vik, sRGB-encoded entries) and its lookup,
/// as `ColourMap::sample_linear` does it.
fn vik_wgsl() -> String {
    let entries: Vec<String> = ColourMap::Vik
        .table()
        .iter()
        .map(|c| format!("vec3<f32>({:?}, {:?}, {:?})", c[0], c[1], c[2]))
        .collect();
    format!(
        "const ON_IMAGE_DIM: f32 = {:?};\n\
         var<private> VIK: array<vec3<f32>, 256> = array<vec3<f32>, 256>({});\n\
         fn vik(t: f32) -> vec3<f32> {{\n\
             let x = clamp(t, 0.0, 1.0) * 255.0;\n\
             let i = min(u32(floor(x)), 254u);\n\
             let e = mix(VIK[i], VIK[i + 1u], x - f32(i));\n\
             return select(pow((e + 0.055) / 1.055, vec3<f32>(2.4)), e / 12.92, e <= vec3<f32>(0.04045));\n\
         }}\n",
        specular_inspect::diff::ON_IMAGE_DIM,
        entries.join(", ")
    )
}

/// The difference view's shader: the shared error function and the fragment pass.
pub(crate) fn diff_source() -> String {
    format!(
        "{FULLSCREEN_WGSL}\n{}\n{}\n{}",
        include_str!("../shaders/compare_error.wgsl"),
        vik_wgsl(),
        include_str!("../shaders/compare.wgsl")
    )
}

/// The metric passes' shader: the error reduction, SSIM and ΔE2000.
pub(crate) fn reduce_source() -> String {
    let w: Vec<String> = ssim_weights().iter().map(|v| format!("{v:?}")).collect();
    format!(
        "{}\n{}\n{}\n{}",
        include_str!("../shaders/compare_error.wgsl"),
        include_str!("../shaders/compare_reduce.wgsl"),
        include_str!("../shaders/compare_ssim.wgsl").replace("SSIM_WEIGHTS", &w.join(", ")),
        include_str!("../shaders/compare_de.wgsl")
    )
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DiffUniform {
    rect: [f32; 4],
    m: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ReduceUniform {
    size: [u32; 2],
    alpha: u32,
    groups: u32,
    slot: u32,
    space: u32,
    _pad: [u32; 2],
}

/// What [`Comparer::draw_diff`] draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiffParams {
    pub kind: DiffKind,
    /// The error at the top of the ramp (`DiffKind::full_scale`).
    pub full_scale: f32,
    /// The overlap of both images in target px (x0, y0, x1, y1); outside it stays clear.
    pub overlap: [f32; 4],
    /// Alpha is a compared channel (either image has alpha and no channel view is on).
    pub alpha: bool,
    /// The space the error is taken in.
    pub space: MetricSpace,
}

/// Where an image sits in the frame compare lays images out in: frame point =
/// `scale` · R(`rotation`) · display point + `offset` (R turns clockwise on screen, y down).
/// Top-left alignment is the identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub scale: f32,
    /// Radians.
    pub rotation: f32,
    pub offset: (f32, f32),
}

impl Default for Place {
    fn default() -> Self {
        Place::IDENTITY
    }
}

impl Place {
    pub const IDENTITY: Place = Place {
        scale: 1.0,
        rotation: 0.0,
        offset: (0.0, 0.0),
    };

    /// Moved by `offset` (frame units), at scale 1.
    pub fn shifted(offset: (f32, f32)) -> Place {
        Place {
            offset,
            ..Place::IDENTITY
        }
    }

    /// The frame point of display point `d`.
    pub fn to_frame(&self, d: (f32, f32)) -> (f32, f32) {
        let (sn, cs) = self.rotation.sin_cos();
        let s = self.scale;
        (
            s * (cs * d.0 - sn * d.1) + self.offset.0,
            s * (sn * d.0 + cs * d.1) + self.offset.1,
        )
    }

    /// The display point at frame point `f`.
    pub fn from_frame(&self, f: (f32, f32)) -> (f32, f32) {
        let (sn, cs) = self.rotation.sin_cos();
        let s = self.scale.max(1e-9);
        let (x, y) = ((f.0 - self.offset.0) / s, (f.1 - self.offset.1) / s);
        (cs * x + sn * y, -sn * x + cs * y)
    }

    /// A still view of an image `stored` px large under `orient` placed so: frame point
    /// `centre` in the middle of `viewport`, `k` viewport px per frame unit.
    pub fn view(
        &self,
        stored: (u32, u32),
        orient: Orientation,
        viewport: PxRect,
        k: f32,
        centre: (f32, f32),
    ) -> View {
        View::fixed(
            stored,
            orient,
            viewport,
            k * self.scale,
            self.from_frame(centre),
        )
        .with_rotation(self.rotation)
    }

    /// The axis-aligned frame rectangle inside the image (display size `size`) as placed:
    /// for a turned image, the box between its corners' second-smallest and second-largest
    /// coordinates (inside it for turns under 45°).
    pub fn inner_rect(&self, size: (f32, f32)) -> (f32, f32, f32, f32) {
        let mut xs = [0f32; 4];
        let mut ys = [0f32; 4];
        for (k, c) in [(0.0, 0.0), (size.0, 0.0), (0.0, size.1), size]
            .into_iter()
            .enumerate()
        {
            (xs[k], ys[k]) = self.to_frame(c);
        }
        xs.sort_by(f32::total_cmp);
        ys.sort_by(f32::total_cmp);
        if self.rotation == 0.0 {
            (xs[0], ys[0], xs[3], ys[3])
        } else {
            (xs[1], ys[1], xs[2], ys[2])
        }
    }
}

/// One image of a metric pair: its id in the [`GpuImageCache`], its orientation and where it
/// sits in the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetricImage {
    pub id: u64,
    pub stored: (u32, u32),
    pub orient: Orientation,
    pub place: Place,
}

/// A pair to measure: the reference `a` and `b`, each where it sits in the frame, over the
/// frame region `region` (x0, y0, x1, y1; inside both images).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetricPair {
    pub a: MetricImage,
    pub b: MetricImage,
    pub region: (f32, f32, f32, f32),
    pub channel: ChannelView,
    pub alpha: bool,
    /// The space MAE, MSE, PSNR and SSIM are taken in (ΔE2000 is always CIELAB).
    pub space: MetricSpace,
}

/// One pair's result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MetricResult {
    pub stats: DiffStats,
    /// Mean SSIM (None: the region is smaller than its 11 px window).
    pub ssim: Option<f64>,
    pub delta_e: DeltaEStats,
    /// Samples taken (the sample target's size).
    pub samples: (u32, u32),
    /// Samples per display pixel (1: every pixel of the region; less: subsampled to fit
    /// [`METRIC_SIZE`]).
    pub density: f32,
}

/// A finished metrics readback.
#[derive(Clone, Debug, Default)]
pub struct MetricsReadback {
    /// In the order of the request's pairs (`None`: nothing of a pair was on the GPU).
    pub pairs: Vec<Option<MetricResult>>,
    /// Milliseconds from recording to reading the result (GPU latency across frames).
    pub latency_ms: f64,
}

enum State {
    Idle,
    Recorded {
        pairs: Vec<Option<MetricResult>>,
        at: std::time::Instant,
    },
    Mapping {
        pairs: Vec<Option<MetricResult>>,
        at: std::time::Instant,
        done: Arc<AtomicU8>,
    },
}

/// The sample targets, as large as the largest request so far needed.
struct Samples {
    size: (u32, u32),
    a: wgpu::TextureView,
    b: wgpu::TextureView,
}

/// The probe's readback: nothing asked, copied, or mapping.
enum ProbeState {
    Idle,
    Recorded(usize),
    Mapping(usize, Arc<AtomicU8>),
}

pub struct Comparer {
    diff_pipe: wgpu::RenderPipeline,
    diff_bgl: wgpu::BindGroupLayout,
    partial_pipe: wgpu::ComputePipeline,
    total_pipe: wgpu::ComputePipeline,
    ssim_pipe: wgpu::ComputePipeline,
    de_pipe: wgpu::ComputePipeline,
    reduce_bgl: wgpu::BindGroupLayout,
    partials: wgpu::Buffer,
    totals: wgpu::Buffer,
    hist: wgpu::Buffer,
    readback: wgpu::Buffer,
    probe: wgpu::Buffer,
    probe_state: ProbeState,
    samples: Option<Samples>,
    state: State,
    /// CPU milliseconds the last `request` spent recording.
    pub last_record_ms: f64,
}

impl std::fmt::Debug for Comparer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Comparer")
            .field("busy", &self.busy())
            .field("samples", &self.samples.as_ref().map(|s| s.size))
            .finish_non_exhaustive()
    }
}

/// A [`Comparer`] being built on a background thread (its pipelines take tens of
/// milliseconds to compile, which would stall the frame that opens the difference view).
pub struct ComparerBuild {
    rx: std::sync::mpsc::Receiver<Comparer>,
    ready: Option<Comparer>,
}

impl std::fmt::Debug for ComparerBuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComparerBuild")
            .field("ready", &self.ready.is_some())
            .finish_non_exhaustive()
    }
}

impl ComparerBuild {
    /// The comparer, once built (never blocks).
    pub fn try_take(&mut self) -> Option<Comparer> {
        self.ready.take().or_else(|| self.rx.try_recv().ok())
    }

    /// Waits for the comparer (tests, screenshots).
    pub fn wait(mut self) -> Option<Comparer> {
        self.ready.take().or_else(|| self.rx.recv().ok())
    }

    /// Whether the build ended without a comparer (it panicked; the log has why). A comparer
    /// that arrives meanwhile is kept for [`try_take`](Self::try_take).
    pub fn failed(&mut self) -> bool {
        if self.ready.is_some() {
            return false;
        }
        match self.rx.try_recv() {
            Ok(c) => {
                self.ready = Some(c);
                false
            }
            Err(e) => e == std::sync::mpsc::TryRecvError::Disconnected,
        }
    }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn compute_tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl Comparer {
    /// Builds a comparer on a background thread; `done` runs on that thread once it is ready
    /// (to wake the event loop). Builds here when no thread can start.
    pub fn spawn(gpu: &Gpu, done: impl FnOnce() + Send + 'static) -> ComparerBuild {
        Self::spawn_on("gpu-compare", gpu, done)
    }

    /// [`spawn`](Self::spawn) on a thread named `thread`. A panic in the build ends it
    /// without a comparer ([`ComparerBuild::failed`]); `done` runs either way.
    pub(crate) fn spawn_on(
        thread: &str,
        gpu: &Gpu,
        done: impl FnOnce() + Send + 'static,
    ) -> ComparerBuild {
        let (tx, rx) = std::sync::mpsc::channel();
        let g = gpu.clone();
        let spawned = specular_thread::builder(thread).spawn(move || {
            let built = specular_core::isolate::run("building the difference view", || {
                specular_log::service::trip();
                Comparer::new(&g)
            });
            if let Ok(c) = built {
                let _ = tx.send(c);
            }
            drop(tx);
            done();
        });
        let ready = spawned.is_err().then(|| Comparer::new(gpu));
        ComparerBuild { rx, ready }
    }

    pub fn new(gpu: &Gpu) -> Comparer {
        let t0 = std::time::Instant::now();
        let d = &gpu.device;
        let module = specular_gfx::shader(d, "compare diff", &diff_source());
        let diff_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compare diff"),
            entries: &[
                super::tex_entry(0, wgpu::TextureViewDimension::D2, false),
                super::tex_entry(1, wgpu::TextureViewDimension::D2, false),
                super::uniform_entry(2),
            ],
        });
        let diff_pipe = fullscreen_pipeline(
            gpu,
            "compare diff",
            &module,
            "fs_diff",
            &[Some(&diff_bgl)],
            HDR,
            None,
        );
        let reduce_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compare reduce"),
            entries: &[
                compute_tex_entry(0),
                compute_tex_entry(1),
                storage_entry(2),
                storage_entry(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage_entry(5),
            ],
        });
        let rmod = specular_gfx::shader(d, "compare reduce", &reduce_source());
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compare reduce"),
            bind_group_layouts: &[Some(&reduce_bgl)],
            immediate_size: 0,
        });
        let compute = |entry: &str| {
            d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &rmod,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: gpu.cache(),
            })
        };
        let partial_pipe = compute("cs_partial");
        let total_pipe = compute("cs_total");
        let ssim_pipe = compute("cs_ssim");
        let de_pipe = compute("cs_de");
        let partials = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compare partials"),
            size: MAX_GROUPS * 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let totals = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compare totals"),
            size: TOTAL_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hist = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compare ΔE histograms"),
            size: HIST_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let readback = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compare readback"),
            size: TOTAL_BYTES + HIST_BYTES,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let probe = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compare probe"),
            size: PROBE_ROW * PROBE_TEXELS as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        specular_log::debug!(target: specular_log::RENDER, "compare: pipelines built in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
        Comparer {
            diff_pipe,
            diff_bgl,
            partial_pipe,
            total_pipe,
            ssim_pipe,
            de_pipe,
            reduce_bgl,
            partials,
            totals,
            hist,
            readback,
            probe,
            probe_state: ProbeState::Idle,
            samples: None,
            state: State::Idle,
            last_record_ms: 0.0,
        }
    }

    /// Draws the difference of `a` and `b` (renders of the same frame, `size` px, over
    /// nothing) into `target` in false colour.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_diff(
        &self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        a: &wgpu::TextureView,
        b: &wgpu::TextureView,
        target: &wgpu::TextureView,
        p: &DiffParams,
    ) {
        let u = DiffUniform {
            rect: p.overlap,
            m: [
                p.kind.shader_mode() as f32,
                1.0 / p.full_scale.max(1e-12),
                if p.alpha { 1.0 } else { 0.0 },
                p.space.shader_flag() as f32,
            ],
        };
        let ub = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("compare diff"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compare diff"),
            layout: &self.diff_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ub.as_entire_binding(),
                },
            ],
        });
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("compare diff"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        rp.set_pipeline(&self.diff_pipe);
        rp.set_bind_group(0, &bind, &[]);
        rp.draw(0..3, 0..1);
    }

    /// A request is recorded or in flight.
    pub fn busy(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Sample targets of at least `w` × `h`.
    fn samples(&mut self, gpu: &Gpu, w: u32, h: u32) -> &Samples {
        let fits = self
            .samples
            .as_ref()
            .is_some_and(|s| s.size.0 >= w && s.size.1 >= h);
        if !fits {
            // Whole 256 px steps, so panning does not reallocate every frame.
            let (sw, sh) = self.samples.as_ref().map_or((0, 0), |s| s.size);
            let w = w.max(sw).div_ceil(256).max(1) * 256;
            let h = h.max(sh).div_ceil(256).max(1) * 256;
            let (w, h) = (w.min(METRIC_SIZE), h.min(METRIC_SIZE));
            let make = |label| {
                super::texture_2d(
                    &gpu.device,
                    label,
                    w,
                    h,
                    1,
                    1,
                    HDR,
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                )
                .create_view(&Default::default())
            };
            self.samples = Some(Samples {
                size: (w, h),
                a: make("compare sample a"),
                b: make("compare sample b"),
            });
        }
        self.samples.as_ref().expect("made above")
    }

    /// Lets go of the sample targets (compare closed); the next request makes them again.
    pub fn trim(&mut self) {
        if !self.busy() {
            self.samples = None;
        }
    }

    /// Bytes of GPU memory the sample targets hold.
    pub fn sample_bytes(&self) -> u64 {
        self.samples
            .as_ref()
            .map_or(0, |s| s.size.0 as u64 * s.size.1 as u64 * 8 * 2)
    }

    /// Records the metrics of up to [`MAX_PAIRS`] `pairs` into `enc`. Returns false (recording
    /// nothing) while an earlier request is in flight or with no pairs. Call
    /// [`Comparer::after_submit`] once `enc` is submitted.
    pub fn request(
        &mut self,
        gpu: &Gpu,
        renderer: &mut ImageRenderer,
        images: &mut GpuImageCache,
        enc: &mut wgpu::CommandEncoder,
        pairs: &[MetricPair],
    ) -> bool {
        if self.busy() || pairs.is_empty() {
            return false;
        }
        let t0 = std::time::Instant::now();
        enc.clear_buffer(&self.totals, 0, None);
        enc.clear_buffer(&self.hist, 0, None);
        let mut results = Vec::with_capacity(pairs.len());
        for (k, p) in pairs.iter().take(MAX_PAIRS).enumerate() {
            let Some((w, h, va, vb)) = metric_views(p) else {
                results.push(None);
                continue;
            };
            let resident = [p.a.id, p.b.id].into_iter().all(|id| {
                images
                    .get(id)
                    .is_some_and(|img| super::inspect::resident(renderer, img))
            });
            if !resident {
                results.push(None);
                continue;
            }
            let opts = RenderOptions {
                hq: false,
                canvas: [0.0; 4],
                checker_a: [0.0; 4],
                checker_b: [0.0; 4],
                checker_cell: 1.0e6,
                grid: None,
                // Nearest everywhere: one texel per sample, never a blend of neighbours.
                nearest_from: 0.0,
                channel: p.channel,
                ..RenderOptions::default()
            };
            let s = self.samples(gpu, w, h);
            let (ta, tb) = (s.a.clone(), s.b.clone());
            for (id, view, target) in [(p.a.id, &va, &ta), (p.b.id, &vb, &tb)] {
                if let Some(img) = images.get(id) {
                    renderer.render(gpu, enc, target, (w, h), Some(img), view, &opts);
                }
            }
            let (gx, gy) = (w.div_ceil(16), h.div_ceil(16));
            // The SSIM centres: every pixel whose 11 px window lies inside.
            let ssim = (w > 10 && h > 10).then(|| ((w - 10).div_ceil(16), (h - 10).div_ceil(16)));
            let mut passes = vec![(&self.partial_pipe, (gx, gy), 0)];
            if let Some(g) = ssim {
                passes.push((&self.ssim_pipe, g, 1));
            }
            passes.push((&self.de_pipe, (gx, gy), 2));
            for (pipe, (px, py), metric) in passes {
                let u = ReduceUniform {
                    size: [w, h],
                    alpha: p.alpha as u32,
                    groups: px * py,
                    slot: (k * SLOTS + metric) as u32,
                    space: p.space.shader_flag(),
                    _pad: [0; 2],
                };
                let ub = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("compare reduce"),
                    contents: bytemuck::bytes_of(&u),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("compare reduce"),
                    layout: &self.reduce_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&ta),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&tb),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.partials.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.totals.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: ub.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: self.hist.as_entire_binding(),
                        },
                    ],
                });
                {
                    let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("compare partials"),
                        timestamp_writes: None,
                    });
                    cp.set_pipeline(pipe);
                    cp.set_bind_group(0, &bind, &[]);
                    cp.dispatch_workgroups(px, py, 1);
                }
                {
                    let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("compare total"),
                        timestamp_writes: None,
                    });
                    cp.set_pipeline(&self.total_pipe);
                    cp.set_bind_group(0, &bind, &[]);
                    cp.dispatch_workgroups(1, 1, 1);
                }
            }
            let (dw, dh) = (p.region.2 - p.region.0, p.region.3 - p.region.1);
            results.push(Some(MetricResult {
                stats: DiffStats {
                    pixels: w as u64 * h as u64,
                    ..DiffStats::default()
                },
                ssim: ssim.map(|_| 0.0),
                delta_e: DeltaEStats::default(),
                samples: (w, h),
                density: (w as f32 / dw).min(h as f32 / dh).min(1.0),
            }));
        }
        if results.iter().all(Option::is_none) {
            return false;
        }
        enc.copy_buffer_to_buffer(&self.totals, 0, &self.readback, 0, TOTAL_BYTES);
        enc.copy_buffer_to_buffer(&self.hist, 0, &self.readback, TOTAL_BYTES, HIST_BYTES);
        self.state = State::Recorded {
            pairs: results,
            at: t0,
        };
        self.last_record_ms = t0.elapsed().as_secs_f64() * 1e3;
        true
    }

    /// Starts mapping the readback once the encoder holding the request is submitted.
    pub fn after_submit(&mut self) {
        let State::Recorded { pairs, at } = std::mem::replace(&mut self.state, State::Idle) else {
            return;
        };
        let done = Arc::new(AtomicU8::new(0));
        let d = done.clone();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                d.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
        self.state = State::Mapping { pairs, at, done };
    }

    /// Drops a recorded request whose encoder was never submitted.
    pub fn cancel_recorded(&mut self) {
        if matches!(self.state, State::Recorded { .. }) {
            self.state = State::Idle;
        }
    }

    /// The finished readback, if the GPU is done with it (never blocks).
    pub fn poll(&mut self, gpu: &Gpu) -> Option<MetricsReadback> {
        let State::Mapping { ref done, .. } = self.state else {
            return None;
        };
        if done.load(Ordering::Acquire) == 0 {
            let _ = gpu.poll(wgpu::PollType::Poll);
        }
        match done.load(Ordering::Acquire) {
            0 => return None,
            1 => {}
            _ => {
                self.state = State::Idle;
                return None;
            }
        }
        let State::Mapping { pairs, at, .. } = std::mem::replace(&mut self.state, State::Idle)
        else {
            return None;
        };
        let out = {
            let data = self.readback.slice(..).get_mapped_range().ok()?;
            let (tot, hist) = data[..].split_at(TOTAL_BYTES as usize);
            let v: &[[f32; 4]] = bytemuck::cast_slice(tot);
            let hist: &[u32] = bytemuck::cast_slice(hist);
            MetricsReadback {
                pairs: pairs
                    .into_iter()
                    .enumerate()
                    .map(|(k, r)| {
                        r.map(|r| {
                            let n = r.stats.pixels;
                            let (s, de) = (v[k * SLOTS + 1], v[k * SLOTS + 2]);
                            let bins = &hist[k * DE_BINS..(k + 1) * DE_BINS];
                            MetricResult {
                                stats: DiffSums::from_gpu(v[k * SLOTS], n).stats(),
                                ssim: r
                                    .ssim
                                    .filter(|_| s[1] > 0.0)
                                    .map(|_| f64::from(s[0]) / f64::from(s[1])),
                                delta_e: DeltaEStats::from_sums(f64::from(de[0]), de[3], bins, n),
                                ..r
                            }
                        })
                    })
                    .collect(),
                latency_ms: at.elapsed().as_secs_f64() * 1e3,
            }
        };
        self.readback.unmap();
        Some(out)
    }
}

/// The sample target size and the views of both images for `p`: its frame region at no more
/// than one sample per frame unit (a display pixel of the first image) and no more than
/// [`METRIC_SIZE`] on a side, each image through its [`Place`]. `None` when the region is
/// empty.
pub fn metric_views(p: &MetricPair) -> Option<(u32, u32, View, View)> {
    let (x0, y0, x1, y1) = p.region;
    let (dw, dh) = (x1 - x0, y1 - y0);
    if dw < 0.5 || dh < 0.5 {
        return None;
    }
    let k = (METRIC_SIZE as f32 / dw.max(dh)).min(1.0);
    let w = ((dw * k).round() as u32).clamp(1, METRIC_SIZE);
    let h = ((dh * k).round() as u32).clamp(1, METRIC_SIZE);
    // Per axis the rounding differs; the larger scale keeps every sample inside the region.
    let k = (w as f32 / dw).max(h as f32 / dh);
    let vp = PxRect {
        x: 0.0,
        y: 0.0,
        w: w as f32,
        h: h as f32,
    };
    let centre = (x0 + w as f32 / k * 0.5, y0 + h as f32 / k * 0.5);
    let view = |m: &MetricImage| m.place.view(m.stored, m.orient, vp, k, centre);
    Some((w, h, view(&p.a), view(&p.b)))
}

/// One texel of an [`HDR`] (half-float RGBA) texture as `f32`s.
fn texel(b: &[u8]) -> [f32; 4] {
    let h = |i: usize| specular_core::hdr::f16_to_f32(u16::from_le_bytes([b[i], b[i + 1]]));
    [h(0), h(2), h(4), h(6)]
}

impl Comparer {
    /// The probe is copied or mapping.
    pub fn probe_busy(&self) -> bool {
        !matches!(self.probe_state, ProbeState::Idle)
    }

    /// Records copies of one texel of each of up to two [`HDR`] textures (the panes' renders,
    /// made with `COPY_SRC`) at physical pixel `at` into `enc`. False while a probe is in
    /// flight. Call [`Comparer::probe_after_submit`] once `enc` is submitted.
    pub fn request_probe(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        texels: &[(&wgpu::Texture, (u32, u32))],
    ) -> bool {
        if self.probe_busy() || texels.is_empty() {
            return false;
        }
        let n = texels.len().min(PROBE_TEXELS);
        for (k, (tex, at)) in texels.iter().take(n).enumerate() {
            let at = (
                at.0.min(tex.width().saturating_sub(1)),
                at.1.min(tex.height().saturating_sub(1)),
            );
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: at.0,
                        y: at.1,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.probe,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: k as u64 * PROBE_ROW,
                        bytes_per_row: Some(PROBE_ROW as u32),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.probe_state = ProbeState::Recorded(n);
        true
    }

    /// Starts mapping the probe once its encoder is submitted.
    pub fn probe_after_submit(&mut self) {
        let ProbeState::Recorded(n) = self.probe_state else {
            return;
        };
        let done = Arc::new(AtomicU8::new(0));
        let d = done.clone();
        self.probe
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                d.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
        self.probe_state = ProbeState::Mapping(n, done);
    }

    /// Drops a probe whose encoder was never submitted.
    pub fn cancel_probe(&mut self) {
        if matches!(self.probe_state, ProbeState::Recorded(_)) {
            self.probe_state = ProbeState::Idle;
        }
    }

    /// The probed texels (premultiplied linear, in the order asked) once the GPU is done
    /// (never blocks).
    pub fn poll_probe(&mut self, gpu: &Gpu) -> Option<Vec<[f32; 4]>> {
        let ProbeState::Mapping(n, ref done) = self.probe_state else {
            return None;
        };
        if done.load(Ordering::Acquire) == 0 {
            let _ = gpu.poll(wgpu::PollType::Poll);
        }
        match done.load(Ordering::Acquire) {
            0 => return None,
            1 => {}
            _ => {
                self.probe_state = ProbeState::Idle;
                return None;
            }
        }
        self.probe_state = ProbeState::Idle;
        let out = {
            let data = self.probe.slice(..).get_mapped_range().ok()?;
            (0..n)
                .map(|k| texel(&data[k * PROBE_ROW as usize..]))
                .collect()
        };
        self.probe.unmap();
        Some(out)
    }
}

#[cfg(test)]
mod tests;
