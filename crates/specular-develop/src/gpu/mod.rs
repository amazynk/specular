//! The device-side engine: runs a [`Graph`] on wgpu compute.
//!
//! - **Kernels** are generated per group ([`wgsl`]) and kept in a pipeline cache keyed by
//!   their source. Param values live in uniform blocks, so a slider rewrites a few bytes and
//!   re-dispatches; it never creates a pipeline ([`Counters::pipelines`]).
//! - **Buffers** come from a pool and are reused last-in first-out; bind groups are cached
//!   by the buffers and uniform slot they bind, so a repeated render creates nothing.
//! - **Checkpoints**: the output of every checkpoint group is cached, keyed by the edit's
//!   prefix hash, the source, region, scale, precision and [`GRAPH_VERSION`]. A render
//!   resumes after the last cached checkpoint.
//! - **Tiles** ([`crate::graph::tile`]) when a request does not fit the budget or a binding.
//!   Every tile is recorded into one command buffer; the last group of each tile writes
//!   straight into the output buffer.
//! - **Statistics**: before the tiles, the source is
//!   reduced to the statistics grid ([`crate::graph::stats_grid`]), the groups before each
//!   node that takes statistics run over it, and the node's statistics program runs on that
//!   input. The result is cached per node by the input's prefix hash and the program's hash,
//!   so a slider on the node itself (or after it) reuses it.
//! - **Local programs** (pyramids of multi-scale nodes) run over each tile's input region
//!   right before the node's kernel, in one scratch buffer from the pool.
//! - **Timestamps**: when the device has `TIMESTAMP_QUERY`, a profiled render reports the
//!   GPU time of every dispatch.
//! - **Masks** ([`crate::mask`]): the graph's mask table lives in one buffer bound to every
//!   dispatch. A render uploads only what changed: the head (programs, grid) when its hash
//!   moves, a brush raster's touched rows after a dab, a whole raster when it is new.
//!
//! The engine borrows the app's device and queue; it never opens a device of its own.

pub mod pool;
pub mod wgsl;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use crate::graph::tile::{self, Limits, Shape, Tiling};
use crate::graph::{
    GRAPH_VERSION, Graph, HEADER_WORDS, OpData, PARAM_WORDS, Precision, Program, Roi, STATS_SIDE,
    stats_grid_at,
};
use crate::model::ModuleKind;
use crate::model::hash::Hash;
use pool::{Budget, Cache, GpuBuf, Key, Pool};
use wgpu::util::DeviceExt;

const PARAM_BYTES: u64 = PARAM_WORDS as u64 * 4;
/// Node tables ([`OpData`]) kept on the GPU.
const TABLES_KEPT: usize = 4;

/// Why a render failed.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The request does not fit the image or is empty.
    BadRequest(&'static str),
    /// Even the smallest tiles need more video memory than the budget allows.
    TooLarge { needs: u64 },
    /// The graph has a node kind this engine does not run yet.
    Unsupported(&'static str),
    /// The device reported an error (validation, out of memory).
    Gpu(String),
    /// The driver reset the GPU: nothing renders on this device any more. The app opens a
    /// new device and starts again ([`Engine::watch_device_loss`]).
    DeviceLost,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadRequest(m) => write!(f, "bad render request: {m}"),
            Error::TooLarge { needs } => {
                write!(
                    f,
                    "needs {} MB of free video memory",
                    needs.div_ceil(1 << 20)
                )
            }
            Error::Unsupported(m) => write!(f, "not supported yet: {m}"),
            Error::Gpu(m) => write!(f, "GPU error: {m}"),
            Error::DeviceLost => write!(f, "the GPU was reset (device lost)"),
        }
    }
}

impl std::error::Error for Error {}

/// An image on the GPU that renders start from (the raw front end's output).
#[derive(Clone, Debug)]
pub struct Source {
    buf: GpuBuf,
    pub width: u32,
    pub height: u32,
    pub precision: Precision,
}

impl Source {
    /// Identifies the pixels in cache keys.
    pub fn id(&self) -> u64 {
        self.buf.id
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buf.buffer
    }
}

/// Where a [`Request::checkpoint_every`] output is kept: the group's last module, the
/// source, the region, the scale's bits and the precision.
type EverySlot = ((crate::model::ModuleKind, u16), u64, Roi, u32, Precision);

/// What to render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    /// The region of the (scaled) source to produce.
    pub roi: Roi,
    /// View scale (1 = source pixels); ops scale their radii by it.
    pub scale: f32,
    pub precision: Precision,
    pub tiling: Tiling,
    /// Keep checkpoint outputs in the cache (off for one-off exports).
    pub checkpoints: bool,
    /// Keep a copy of the result in the cache too (with `checkpoints`): the same render
    /// again (undo, redo) is a copy.
    pub keep_result: bool,
    /// Measure every dispatch with GPU timestamps (when the device has them).
    pub profile: bool,
    /// Keep every group's output as a checkpoint (with `checkpoints`), not only after
    /// heavy groups: a change to a later module resumes right before its group.
    /// Whole-picture previews only: the tile plan does not count the extra outputs.
    pub checkpoint_every: bool,
}

impl Request {
    /// An interactive request: f16 storage, checkpoints, tiles when needed.
    pub fn new(roi: Roi) -> Request {
        Request {
            roi,
            scale: 1.0,
            precision: Precision::F16,
            tiling: Tiling::Auto,
            checkpoints: true,
            keep_result: false,
            profile: false,
            checkpoint_every: false,
        }
    }
}

/// What one render did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderStats {
    pub tiles: usize,
    pub tile_side: Option<u32>,
    pub dispatches: usize,
    /// The modules whose kernels ran, in order (once per tile).
    pub nodes_run: Vec<(ModuleKind, u16)>,
    /// Tiles that resumed from a cached checkpoint.
    pub cache_hits: usize,
    /// The whole result was a copy of a kept one ([`Request::keep_result`]).
    pub result_hit: bool,
    pub pipelines_created: u64,
    pub bind_groups_created: u64,
    pub buffers_created: u64,
    /// Bytes of uniforms written.
    pub uniform_bytes: u64,
    /// GPU milliseconds per dispatch (label, ms), when profiled.
    pub gpu_ms: Option<Vec<(String, f64)>>,
    /// Dispatches in each queue submission, in order ([`Engine::set_submission_ms`]).
    pub submissions: Vec<usize>,
}

impl RenderStats {
    /// Total GPU milliseconds, when profiled.
    pub fn gpu_total_ms(&self) -> Option<f64> {
        self.gpu_ms
            .as_ref()
            .map(|v| v.iter().map(|(_, ms)| ms).sum())
    }
}

/// A rendered region: `roi.w × roi.h` pixels, rows packed, at `precision`.
#[derive(Debug)]
pub struct Rendered {
    buf: GpuBuf,
    pub roi: Roi,
    pub precision: Precision,
    pub stats: RenderStats,
}

impl Rendered {
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buf.buffer
    }

    /// Bytes of pixel data at the start of [`Rendered::buffer`].
    pub fn bytes(&self) -> u64 {
        self.roi.pixels() * self.precision.bytes_per_pixel()
    }
}

/// Everything the engine has done since it was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub pipelines: u64,
    pub bind_groups: u64,
    pub buffers: u64,
    pub dispatches: u64,
    pub renders: u64,
}

/// A buffer region as a kernel sees it (see [`wgsl`]).
#[derive(Clone, Debug)]
struct View {
    buf: GpuBuf,
    offset: u64,
    size: u64,
    ox: u32,
    oy: u32,
    stride: u32,
    base: u32,
    precision: Precision,
}

impl View {
    /// The whole of a packed buffer holding `r`.
    fn packed(buf: GpuBuf, r: Roi, precision: Precision) -> View {
        let size = (r.pixels() * precision.bytes_per_pixel()).max(16);
        View {
            buf,
            offset: 0,
            size,
            ox: r.x,
            oy: r.y,
            stride: r.w,
            base: 0,
            precision,
        }
    }

    /// A whole buffer as f32 words (statistics, scratch).
    fn words(buf: GpuBuf) -> View {
        View {
            offset: 0,
            size: buf.size,
            buf,
            ox: 0,
            oy: 0,
            stride: 0,
            base: 0,
            precision: Precision::F32,
        }
    }

    /// Rows `y0..y1` (image coordinates) of a buffer holding `whole`, rows packed.
    fn rows(buf: GpuBuf, whole: Roi, y0: u32, y1: u32, precision: Precision, align: u64) -> View {
        let bpp = precision.bytes_per_pixel();
        let first = u64::from(y0 - whole.y) * u64::from(whole.w) * bpp;
        let offset = first / align * align;
        let skip = first - offset;
        let size = (skip + u64::from(y1 - y0) * u64::from(whole.w) * bpp).max(16);
        View {
            buf,
            offset,
            size,
            ox: whole.x,
            oy: y0,
            stride: whole.w,
            base: (skip / bpp) as u32,
            precision,
        }
    }

    fn header(&self) -> [u32; 4] {
        [self.ox, self.oy, self.stride, self.base]
    }

    fn key(&self) -> (u64, u64, u64) {
        (self.buf.id, self.offset, self.size)
    }
}

type ViewKey = (u64, u64, u64);
/// Uniform slot, the five views and the mask buffer.
type BindKey = (u64, [ViewKey; 5], u64);

/// The mask table on the GPU.
struct MaskGpu {
    buf: GpuBuf,
    /// Words the buffer holds.
    cap: u64,
    head_hash: Option<Hash>,
    head_words: usize,
    /// Per raster slot: offset, raster id and revision uploaded.
    rasters: Vec<(u64, u64, u64)>,
}

/// One dispatch as planned: its kernel, uniform slot (its index), bindings and threads.
struct Planned {
    code: String,
    /// Destination (or a step's scratch), source, statistics, local scratch, table.
    views: [View; 5],
    threads: [u32; 2],
    label: String,
    slot: u64,
}

/// A render's dispatches and their uniform blocks, in order.
///
/// Each dispatch takes uniform slot `next`, then `next` moves on. The tiles' dispatches
/// get slots that depend only on the graph and the request (not on what the cache held),
/// and statistics come after them, so a repeated render binds the same slots and finds its
/// bind groups cached.
#[derive(Default)]
struct Rec {
    uniforms: Vec<u32>,
    planned: Vec<Planned>,
    next: u64,
}

impl Rec {
    /// Adds a dispatch; returns its uniform block to fill.
    fn push(&mut self, mut p: Planned) -> &mut [u32] {
        p.slot = self.next;
        self.next += 1;
        let at = p.slot as usize * PARAM_WORDS as usize;
        self.planned.push(p);
        if self.uniforms.len() < at + PARAM_WORDS as usize {
            self.uniforms.resize(at + PARAM_WORDS as usize, 0);
        }
        &mut self.uniforms[at..at + PARAM_WORDS as usize]
    }
}

/// Cached statistics of one node on one source and grid.
struct Stats {
    /// The input's prefix hash and the program's hash.
    key: (Hash, Hash),
    buf: GpuBuf,
}

type StatsSlot = ((ModuleKind, u16), u64, u32, u32);

/// One recorded dispatch.
struct Dispatch {
    pipeline: wgpu::ComputePipeline,
    bind: wgpu::BindGroup,
    groups: [u32; 2],
    label: String,
    /// Threads it runs (its cost estimate's unit).
    threads: u64,
}

/// The graph engine on one device.
pub struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    max_binding: u64,
    offset_align: u64,
    max_groups: u32,
    timestamps: bool,
    budget: Budget,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: HashMap<String, wgpu::ComputePipeline>,
    bind_groups: HashMap<BindKey, wgpu::BindGroup>,
    uniforms: Option<(wgpu::Buffer, u64)>,
    pool: Pool,
    cache: Cache,
    /// The newest checkpoint kept by [`Request::checkpoint_every`] per place (the group's
    /// last module, source, region, scale, precision): an older edit's output there goes
    /// back to the pool, so moving a slider reuses its buffers.
    every: HashMap<EverySlot, pool::Key>,
    stats: HashMap<StatsSlot, Stats>,
    /// The statistics grid of a source: (source id, width, height, pixels).
    grid: Option<(u64, u32, u32, GpuBuf)>,
    /// Bound where a kernel reads no statistics or local scratch.
    dummy: GpuBuf,
    /// Node tables by content hash, least recently used first.
    tables: Vec<(Hash, GpuBuf)>,
    /// The mask table (the dummy until a render has masks).
    masks: Option<MaskGpu>,
    /// Scratch of persistent local programs ([`crate::graph::Persist`]), least recently
    /// used first, with the hash of the program that last wrote it when it may be reused
    /// as it is ([`crate::graph::Persist::reuse`]).
    kept: Vec<(KeptKey, GpuBuf, Option<Hash>)>,
    next_id: u64,
    counters: Counters,
    /// GPU nanoseconds per thread of each kernel (by label), from profiled renders.
    speeds: HashMap<String, f64>,
    /// Set when the device is lost ([`Engine::watch_device_loss`]).
    lost: Arc<AtomicBool>,
    /// Estimated GPU milliseconds one submission may hold.
    submit_ms: f64,
    /// Long side of the statistics grid ([`STATS_SIDE`] unless set).
    stats_side: u32,
    /// Runs of point groups as one kernel ([`run_end`]); on unless a test or bench
    /// compares without.
    pub point_runs: bool,
}

/// Persistent scratch kept at most (nodes × tiles).
const KEPT: usize = 4;

/// What a persistent local program's scratch was made for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct KeptKey {
    node: (ModuleKind, u16),
    /// The node's input (its prefix hash and source), region, scale and precision.
    input: Key,
    words: u64,
}

impl Engine {
    /// An engine on the app's device. Uses timestamp queries when the device was opened
    /// with `TIMESTAMP_QUERY`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, budget: Budget) -> Engine {
        let limits = device.limits();
        let buffer =
            |binding: u32, ty: wgpu::BufferBindingType, min: u64| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(min),
                },
                count: None,
            };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("develop kernel"),
            entries: &[
                buffer(0, wgpu::BufferBindingType::Uniform, PARAM_BYTES),
                buffer(1, wgpu::BufferBindingType::Storage { read_only: false }, 0),
                buffer(2, wgpu::BufferBindingType::Storage { read_only: true }, 0),
                buffer(3, wgpu::BufferBindingType::Storage { read_only: true }, 0),
                buffer(4, wgpu::BufferBindingType::Storage { read_only: true }, 0),
                buffer(5, wgpu::BufferBindingType::Storage { read_only: true }, 0),
                buffer(6, wgpu::BufferBindingType::Storage { read_only: true }, 0),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop kernel"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let dummy = GpuBuf {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop no statistics"),
                size: 16,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            id: 0,
            size: 16,
        };
        Engine {
            device: device.clone(),
            queue: queue.clone(),
            max_binding: limits
                .max_storage_buffer_binding_size
                .min(limits.max_buffer_size),
            offset_align: u64::from(limits.min_storage_buffer_offset_alignment).max(16),
            max_groups: limits.max_compute_workgroups_per_dimension,
            timestamps: device.features().contains(wgpu::Features::TIMESTAMP_QUERY),
            budget,
            layout,
            pipeline_layout,
            pipelines: HashMap::new(),
            bind_groups: HashMap::new(),
            uniforms: None,
            pool: Pool::with_max_buffer(limits.max_buffer_size),
            cache: Cache::default(),
            every: HashMap::new(),
            stats: HashMap::new(),
            grid: None,
            dummy,
            tables: Vec::new(),
            masks: None,
            kept: Vec::new(),
            next_id: 0,
            counters: Counters::default(),
            speeds: HashMap::new(),
            submit_ms: SUBMIT_MS,
            stats_side: STATS_SIDE,
            point_runs: true,
            lost: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// Watches `flag`, which the device's owner sets from its device-lost callback (wgpu
    /// keeps one callback per device, so the engine does not install its own): once set,
    /// every call fails with [`Error::DeviceLost`] at once instead of waiting on the device.
    pub fn watch_device_loss(&mut self, flag: Arc<AtomicBool>) {
        self.lost = flag;
    }

    /// Marks the device lost (a caller's poll found it so).
    pub fn mark_lost(&self) {
        self.lost.store(true, Ordering::Release);
    }

    /// Whether the device was lost.
    pub fn device_lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    /// `e`, or [`Error::DeviceLost`] when the device was lost meanwhile (a lost device
    /// reports its failures as validation or mapping errors).
    fn or_lost<T>(&self, r: Result<T, Error>) -> Result<T, Error> {
        match r {
            Err(Error::Gpu(m)) if self.device_lost() || lost_message(&m) => {
                self.lost.store(true, Ordering::Release);
                Err(Error::DeviceLost)
            }
            Err(Error::DeviceLost) => {
                self.lost.store(true, Ordering::Release);
                Err(Error::DeviceLost)
            }
            r => r,
        }
    }

    /// The statistics grid's long side for the next renders ([`STATS_SIDE`] by default;
    /// smaller for quick drafts).
    pub fn set_stats_side(&mut self, side: u32) {
        let side = side.max(16);
        if side != self.stats_side {
            // Kept scratch may hold values from statistics on the other grid.
            self.trim_kept(0);
        }
        self.stats_side = side;
    }

    /// Sets the estimated GPU milliseconds one queue submission may hold (default
    /// [`SUBMIT_MS`]); 0 submits every dispatch on its own.
    pub fn set_submission_ms(&mut self, ms: f64) {
        self.submit_ms = ms.max(0.0);
    }

    /// The estimated GPU milliseconds of `threads` threads of kernel `label`.
    pub fn estimate_ms(&self, label: &str, threads: u64) -> f64 {
        estimate(&self.speeds, label, threads)
    }

    pub fn has_timestamps(&self) -> bool {
        self.timestamps
    }

    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// Checkpoints in the cache and their bytes.
    pub fn cached(&self) -> (usize, u64) {
        (self.cache.len(), self.cache.bytes)
    }

    /// Drops the kept checkpoints (statistics stay): exports that render several
    /// renditions of a band free the band's shared groups with it.
    pub fn drop_checkpoints(&mut self) {
        for b in self.cache.trim(0) {
            self.pool.release(b);
        }
    }

    /// Bytes held: (kept scratch of persistent programs, free buffers in the pool, every
    /// buffer the pool made and has not destroyed).
    pub fn memory(&self) -> (u64, u64, u64) {
        let kept = self.kept.iter().map(|e| e.1.size).sum();
        (kept, self.pool.free_bytes(), self.pool.live)
    }

    /// Gives back what only speeds up the next render (memory pressure, Develop left):
    /// checkpoints, statistics, kept scratch and every free buffer.
    pub fn trim_memory(&mut self) {
        self.clear_cache();
    }

    /// Releases kept scratch, least recently used first, until at most `cap` bytes remain.
    fn trim_kept(&mut self, cap: u64) {
        let mut kept: u64 = self.kept.iter().map(|e| e.1.size).sum();
        while kept > cap && !self.kept.is_empty() {
            let (_, b, _) = self.kept.remove(0);
            kept -= b.size;
            self.pool.release(b);
        }
    }

    /// Destroys free buffers, oldest first, until at most `cap` bytes are free.
    fn trim_free(&mut self, cap: u64) {
        let gone = self.pool.trim(cap);
        if !gone.is_empty() {
            self.bind_groups
                .retain(|(_, vs, _), _| vs.iter().all(|v| !gone.contains(&v.0)));
        }
    }

    /// Drops every checkpoint, statistic and kept scratch and destroys the free buffers
    /// (device loss, memory pressure).
    pub fn clear_cache(&mut self) {
        self.forget();
        self.trim_free(0);
    }

    /// Drops every checkpoint, statistic and kept scratch (a new source or warp): their
    /// buffers go to the free pool, which keeps what [`Budget::free`] allows, so the next
    /// picture of the same size makes no buffers or bind groups.
    pub fn forget(&mut self) {
        self.every.clear();
        for b in self.cache.trim(0) {
            self.pool.release(b);
        }
        for (_, b, _) in std::mem::take(&mut self.kept) {
            self.pool.release(b);
        }
        for (_, st) in self.stats.drain() {
            self.pool.release(st.buf);
        }
        if let Some((.., b)) = self.grid.take() {
            self.pool.release(b);
        }
        self.trim_free(self.budget.free());
    }

    /// Statistics kept, one per node, source and grid.
    pub fn cached_stats(&self) -> usize {
        self.stats.len()
    }

    /// Uploads rgba32float pixels (`width × height × 4` values) as a source.
    pub fn upload(&mut self, width: u32, height: u32, rgba: &[f32]) -> Result<Source, Error> {
        let n = u64::from(width) * u64::from(height);
        if n == 0 || rgba.len() as u64 != n * 4 {
            return Err(Error::BadRequest("the pixels do not match the size"));
        }
        let bytes = n * 16;
        let buf = self.new_buffer(bytes, "develop source")?;
        self.queue
            .write_buffer(&buf.buffer, 0, bytemuck::cast_slice(rgba));
        Ok(Source {
            buf,
            width,
            height,
            precision: Precision::F32,
        })
    }

    /// Wraps a buffer already on the device (rows packed, `width × height` pixels at
    /// `precision`, usage `STORAGE`) as a source.
    pub fn source_from_buffer(
        &mut self,
        buffer: wgpu::Buffer,
        width: u32,
        height: u32,
        precision: Precision,
    ) -> Result<Source, Error> {
        let need = u64::from(width) * u64::from(height) * precision.bytes_per_pixel();
        if need == 0 || buffer.size() < need {
            return Err(Error::BadRequest("the buffer is smaller than the image"));
        }
        self.next_id += 1;
        Ok(Source {
            buf: GpuBuf {
                size: buffer.size(),
                buffer,
                id: self.next_id,
            },
            width,
            height,
            precision,
        })
    }

    fn new_buffer(&mut self, bytes: u64, label: &str) -> Result<GpuBuf, Error> {
        self.next_id += 1;
        self.counters.buffers += 1;
        let size = bytes.div_ceil(16) * 16;
        let device = &self.device;
        let buffer = scoped(device, || {
            Ok(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }))
        })?;
        Ok(GpuBuf {
            buffer,
            id: self.next_id,
            size,
        })
    }

    /// A pool buffer for a `roi`-sized result at `precision`, for the caller to fill
    /// (a draft scaled up); give it back with [`Engine::recycle`].
    pub fn blank(&mut self, roi: Roi, precision: Precision) -> Rendered {
        let buf = self.pool.acquire(
            &self.device,
            &mut self.next_id,
            roi.pixels() * precision.bytes_per_pixel(),
        );
        Rendered {
            buf,
            roi,
            precision,
            stats: RenderStats::default(),
        }
    }

    /// Gives a rendered buffer back for reuse (the next render of the same size binds it
    /// again without new bind groups).
    pub fn recycle(&mut self, r: Rendered) {
        self.pool.release(r.buf);
    }

    /// Renders `req` of `graph` from `src`.
    pub fn render(
        &mut self,
        graph: &Graph,
        src: &Source,
        req: &Request,
    ) -> Result<Rendered, Error> {
        if req.roi.is_empty()
            || req.roi.right() > src.width
            || req.roi.bottom() > src.height
            || req.scale.is_nan()
            || req.scale <= 0.0
        {
            return Err(Error::BadRequest(
                "the region is empty or outside the image",
            ));
        }
        if self.device_lost() {
            return Err(Error::DeviceLost);
        }
        let device = self.device.clone();
        let r = scoped(&device, || self.render_inner(graph, src, req));
        self.or_lost(r)
    }

    fn render_inner(
        &mut self,
        graph: &Graph,
        src: &Source,
        req: &Request,
    ) -> Result<Rendered, Error> {
        let before = self.counters;
        let pool_created = self.pool.created;
        let prec = req.precision;
        if let Some(t) = &graph.masks {
            self.upload_masks(t);
        }
        // Scratch of a node this edit of the same source no longer runs (turned off or
        // removed) goes.
        let gone: Vec<usize> = (0..self.kept.len())
            .filter(|&i| {
                let k = &self.kept[i].0;
                k.input.source == src.id() && !graph.nodes.iter().any(|nd| nd.id == k.node)
            })
            .collect();
        for i in gone.into_iter().rev() {
            let (_, b, _) = self.kept.remove(i);
            self.pool.release(b);
        }
        let shape = Shape {
            out: req.roi,
            width: src.width,
            height: src.height,
            scale: req.scale,
            precision: prec,
            source_bpp: src.precision.bytes_per_pixel(),
        };
        let limits = Limits {
            max_binding: self.max_binding,
            offset_align: self.offset_align,
            working: self.budget.working(),
        };
        let plan = tile::plan(graph, &shape, req.tiling, &limits)
            .map_err(|t| Error::TooLarge { needs: t.needs })?;
        let n = graph.groups.len();
        let out_bytes = req.roi.pixels() * prec.bytes_per_pixel();
        let out = self
            .pool
            .acquire(&self.device, &mut self.next_id, out_bytes);
        // The whole result, kept: the graph's hash covers every node, mask and table.
        let result_key = (req.checkpoints && req.keep_result && n > 0).then(|| Key {
            prefix: graph.group_hash(n - 1),
            source: src.id(),
            roi: req.roi,
            scale_bits: req.scale.to_bits(),
            precision: prec,
            version: GRAPH_VERSION ^ 0x8000_0000,
        });
        if let Some(k) = &result_key
            && let Some(b) = self.cache.get(k)
        {
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("develop kept result"),
                });
            enc.copy_buffer_to_buffer(&b.buffer, 0, &out.buffer, 0, out_bytes);
            self.queue.submit([enc.finish()]);
            self.counters.renders += 1;
            return Ok(Rendered {
                buf: out,
                roi: req.roi,
                precision: prec,
                stats: RenderStats {
                    result_hit: true,
                    gpu_ms: (req.profile && self.timestamps).then(Vec::new),
                    ..RenderStats::default()
                },
            });
        }
        let mut stats = RenderStats {
            tiles: plan.tiles.len(),
            tile_side: plan.side,
            ..RenderStats::default()
        };
        // Slots: per tile, per group its local program's steps and its kernel.
        let mut bases: Vec<Vec<u64>> = Vec::with_capacity(plan.tiles.len());
        let mut next = 0u64;
        for t in &plan.tiles {
            let rois = graph.rois(*t, src.width, src.height, req.scale);
            let mut b = Vec::with_capacity(n.max(1));
            for (g, r) in rois.iter().enumerate() {
                b.push(next);
                next += 1 + graph.local(g, r.input, req.scale).map_or(0, |p| {
                    (p.steps.len() + p.persist.as_ref().map_or(0, |k| k.reset.len())) as u64
                });
            }
            if n == 0 {
                b.push(next);
                next += 1;
            }
            bases.push(b);
        }
        let mut rec = Rec {
            next,
            ..Rec::default()
        };
        let st = self.statistics(graph, src, req.scale, &mut rec)?;
        let scale_bits = req.scale.to_bits();
        let key = |g: usize, roi: Roi| Key {
            prefix: graph.group_hash(g),
            source: src.id(),
            roi,
            scale_bits,
            precision: prec,
            version: GRAPH_VERSION,
        };
        // Group params do not depend on the tile.
        let params: Vec<Vec<u32>> = (0..n).map(|g| graph.params(g, req.scale)).collect();
        let whole_src = Roi::full(src.width, src.height);
        let img = (src.width, src.height);
        for (t, tile_roi) in plan.tiles.iter().enumerate() {
            let rois = graph.rois(*tile_roi, src.width, src.height, req.scale);
            let mut start = 0;
            let mut cur: Option<(View, bool)> = None; // (view, owned by this render)
            if req.checkpoints {
                for g in (0..n.saturating_sub(1)).rev() {
                    if !(graph.groups[g].checkpoint || req.checkpoint_every) {
                        continue;
                    }
                    if let Some(b) = self.cache.get(&key(g, rois[g].output)) {
                        cur = Some((View::packed(b, rois[g].output, prec), false));
                        start = g + 1;
                        stats.cache_hits += 1;
                        break;
                    }
                }
            }
            let align = self.offset_align;
            let out_view = |y0: u32, y1: u32| View::rows(out.clone(), req.roi, y0, y1, prec, align);
            if n == 0 {
                let s = View::rows(
                    src.buf.clone(),
                    whole_src,
                    tile_roi.y,
                    tile_roi.bottom(),
                    src.precision,
                    align,
                );
                let d = out_view(tile_roi.y, tile_roi.bottom());
                let code = wgsl::copy_source(s.precision, prec);
                rec.next = bases[t][0];
                let w = rec.push(Planned {
                    code,
                    views: [d.clone(), s.clone(), self.none(), self.none(), self.none()],
                    threads: [tile_roi.w, tile_roi.h],
                    label: "copy".into(),
                    slot: 0,
                });
                header(w, *tile_roi, img, &s, Some(&d), *tile_roi, &[]);
                continue;
            }
            let mut from = start;
            while from < n {
                // A run of point groups is one kernel (their outputs in between are
                // neither stored nor kept).
                let g = run_end(graph, &rois, &params, from, self.point_runs);
                let r = crate::graph::GroupRoi {
                    input: rois[from].input,
                    output: rois[g].output,
                };
                let joined: Vec<u32>;
                let group_params = if from == g {
                    &params[g]
                } else {
                    joined = params[from..=g].concat();
                    &joined
                };
                let s = match &cur {
                    Some((v, _)) => v.clone(),
                    None => View::rows(
                        src.buf.clone(),
                        whole_src,
                        r.input.y,
                        r.input.bottom(),
                        src.precision,
                        align,
                    ),
                };
                let last = g + 1 == n;
                let d = if last {
                    out_view(r.output.y, r.output.bottom())
                } else {
                    let b = self.pool.acquire(
                        &self.device,
                        &mut self.next_id,
                        r.output.pixels() * prec.bytes_per_pixel(),
                    );
                    View::packed(b, r.output, prec)
                };
                let job = GroupJob {
                    g,
                    from,
                    params: group_params,
                    input: &s,
                    output: &d,
                    img,
                    region: r,
                    scale: req.scale,
                    stats: (from..=g).find_map(|i| st[i].as_ref()),
                    // Persistent scratch only for interactive renders.
                    keep: req.checkpoints.then(|| Key {
                        prefix: graph.input_hash(from),
                        roi: r.input,
                        ..key(from, r.input)
                    }),
                };
                // The run's first group reserved the slots of its program and kernel.
                rec.next = bases[t][from];
                self.plan_group(&mut rec, graph, &job)?;
                stats.nodes_run.extend(
                    graph.nodes[graph.groups[from].nodes.start..graph.groups[g].nodes.end]
                        .iter()
                        .map(|nd| nd.id),
                );
                // The input is no longer needed (later commands reuse it safely: one queue,
                // in order).
                if let Some((v, true)) = cur.take() {
                    self.pool.release(v.buf);
                }
                let keep = !last
                    && req.checkpoints
                    && (graph.groups[g].checkpoint || req.checkpoint_every);
                if keep && let Some(old) = self.cache.insert(key(g, r.output), d.buf.clone()) {
                    self.pool.release(old);
                }
                if keep && !graph.groups[g].checkpoint {
                    let last_node = graph.nodes[graph.groups[g].nodes.end - 1].id;
                    let slot = (last_node, src.id(), r.output, scale_bits, prec);
                    let k = key(g, r.output);
                    if let Some(old) = self.every.insert(slot, k)
                        && old != k
                        && let Some(b) = self.cache.remove(&old)
                    {
                        self.pool.release(b);
                    }
                }
                cur = if last { None } else { Some((d, !keep)) };
                from = g + 1;
            }
        }
        let dispatches = self.materialize(&rec)?;
        let uniforms = rec.uniforms;
        // Uniforms, then every dispatch in one command buffer.
        let used = uniforms.len() as u64 * 4;
        if let Some((u, _)) = &self.uniforms {
            self.queue
                .write_buffer(u, 0, bytemuck::cast_slice(&uniforms));
        }
        stats.uniform_bytes = used;
        let queries = u32::try_from(dispatches.len() * 2)
            .ok()
            .filter(|&q| q > 0 && q <= wgpu::QUERY_SET_MAX_QUERIES);
        let profile = match queries {
            Some(q) if req.profile && self.timestamps => Some((
                q,
                self.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("develop profile"),
                    ty: wgpu::QueryType::Timestamp,
                    count: q,
                }),
            )),
            _ => None,
        };
        let readback_bufs = profile.as_ref().map(|(q, _)| {
            let bytes = u64::from(*q) * 8;
            let resolve = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop profile"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop profile read"),
                size: bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            ((resolve, read), bytes)
        });
        // Several submissions of at most `submit_ms` of estimated GPU time each: a driver
        // resets a device whose one submission runs too long (more readily under load).
        let parts = submissions(&dispatches, &self.speeds, self.submit_ms);
        stats.submissions = parts.iter().map(|r| r.len()).collect();
        let last = parts.len().saturating_sub(1);
        for (k, range) in parts.into_iter().enumerate() {
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("develop render"),
                });
            let qs = profile.as_ref().map(|(_, qs)| (qs, range.start));
            record(&mut enc, &dispatches[range], qs);
            if k == last
                && let Some((read, bytes)) = &readback_bufs
                && let Some((q, qs)) = &profile
            {
                enc.resolve_query_set(qs, 0..*q, &read.0, 0);
                enc.copy_buffer_to_buffer(&read.0, 0, &read.1, 0, *bytes);
            }
            if k == last
                && let Some(key) = result_key
            {
                let keep = self
                    .pool
                    .acquire(&self.device, &mut self.next_id, out_bytes);
                enc.copy_buffer_to_buffer(&out.buffer, 0, &keep.buffer, 0, out_bytes);
                if let Some(old) = self.cache.insert(key, keep) {
                    self.pool.release(old);
                }
            }
            self.queue.submit([enc.finish()]);
        }
        let readback = readback_bufs.map(|((_, read), bytes)| (read, bytes));
        if let Some((read, bytes)) = readback {
            let ticks: Vec<u64> = {
                let view = map_read(&self.device, &read, bytes)?;
                bytemuck::pod_collect_to_vec(&view[..])
            };
            read.unmap();
            let period = f64::from(self.queue.get_timestamp_period()) * 1e-6;
            stats.gpu_ms = Some(
                dispatches
                    .iter()
                    .enumerate()
                    .map(|(i, d)| {
                        let dt = ticks[2 * i + 1].saturating_sub(ticks[2 * i]);
                        (d.label.clone(), dt as f64 * period)
                    })
                    .collect(),
            );
            if let Some(ms) = &stats.gpu_ms {
                for (d, (_, t)) in dispatches.iter().zip(ms) {
                    learn(&mut self.speeds, d, *t);
                }
            }
        }
        // Keep the cache and the free pool within the budget.
        for b in self.cache.trim(self.budget.cache()) {
            self.pool.release(b);
        }
        // Kept scratch within its cap; free buffers beyond theirs go back to the driver.
        self.trim_kept(self.budget.kept());
        self.trim_free(self.budget.free());
        self.counters.buffers += self.pool.created - pool_created;
        self.counters.dispatches += dispatches.len() as u64;
        self.counters.renders += 1;
        stats.dispatches = dispatches.len();
        stats.pipelines_created = self.counters.pipelines - before.pipelines;
        stats.bind_groups_created = self.counters.bind_groups - before.bind_groups;
        stats.buffers_created = self.counters.buffers - before.buffers;
        Ok(Rendered {
            buf: out,
            roi: req.roi,
            precision: prec,
            stats,
        })
    }

    /// Brings the GPU mask table to `t`, uploading only what changed.
    fn upload_masks(&mut self, t: &crate::mask::MaskTable) {
        let words = t.words.max(4);
        if self.masks.as_ref().is_none_or(|m| m.cap < words) {
            let cap = words.next_power_of_two().max(1 << 14);
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop masks"),
                size: cap * 4,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.next_id += 1;
            self.counters.buffers += 1;
            if let Some(old) = self.masks.take() {
                let id = old.buf.id;
                self.bind_groups.retain(|(_, _, m), _| *m != id);
            }
            self.masks = Some(MaskGpu {
                buf: GpuBuf {
                    size: buffer.size(),
                    buffer,
                    id: self.next_id,
                },
                cap,
                head_hash: None,
                head_words: 0,
                rasters: Vec::new(),
            });
        }
        let m = self.masks.as_mut().expect("mask buffer made");
        let q = &self.queue;
        if m.head_hash != Some(t.head_hash) {
            // Only the words that differ in length need writing past the old head; the
            // whole head is small (programs and the grid).
            q.write_buffer(&m.buf.buffer, 0, bytemuck::cast_slice(&t.head));
            m.head_hash = Some(t.head_hash);
            m.head_words = t.head.len();
        }
        m.rasters.resize(t.rasters.len(), (u64::MAX, 0, 0));
        for (slot, (off, r)) in t.rasters.iter().enumerate() {
            let have = m.rasters[slot];
            if have == (*off, r.id, r.rev) {
                continue;
            }
            let row_bytes = u64::from(r.stride) * 2;
            let rows = match r.dirty {
                Some(d) if have == (*off, r.id, d.from) => d.y0..d.y1,
                _ => 0..r.h,
            };
            if !rows.is_empty() {
                let a = rows.start as usize * r.stride as usize;
                let b = rows.end as usize * r.stride as usize;
                q.write_buffer(
                    &m.buf.buffer,
                    off * 4 + u64::from(rows.start) * row_bytes,
                    bytemuck::cast_slice(&r.data[a..b]),
                );
            }
            m.rasters[slot] = (*off, r.id, r.rev);
        }
    }

    /// The dummy binding (no statistics or local scratch).
    fn none(&self) -> View {
        View::words(self.dummy.clone())
    }

    /// The GPU copy of a node's table, uploaded on first use and kept (the last
    /// [`TABLES_KEPT`] by use).
    fn table(&mut self, d: &OpData) -> GpuBuf {
        if let Some(i) = self.tables.iter().position(|(h, _)| *h == d.hash) {
            let e = self.tables.remove(i);
            let b = e.1.clone();
            self.tables.push(e);
            return b;
        }
        if self.tables.len() >= TABLES_KEPT {
            let (_, old) = self.tables.remove(0);
            self.bind_groups
                .retain(|(_, vs, _), _| vs.iter().all(|v| v.0 != old.id));
        }
        let words: &[f32] = if d.words.is_empty() {
            &[0.0; 4]
        } else {
            &d.words
        };
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("develop table"),
                contents: bytemuck::cast_slice(words),
                usage: wgpu::BufferUsages::STORAGE,
            });
        self.next_id += 1;
        self.counters.buffers += 1;
        let b = GpuBuf {
            size: buffer.size(),
            buffer,
            id: self.next_id,
        };
        self.tables.push((d.hash, b.clone()));
        b
    }

    /// Plans group `job.g`: its local program (if any), then its kernel.
    fn plan_group(
        &mut self,
        rec: &mut Rec,
        graph: &Graph,
        job: &GroupJob<'_>,
    ) -> Result<(), Error> {
        let g = job.g;
        let st = job
            .stats
            .map_or_else(|| self.none(), |b| View::words(b.clone()));
        // The run's first group holds its local program (a kernel node leads it).
        let local = graph.local(job.from, job.region.input, job.scale);
        let run = job.from..g + 1;
        let dtv = match run.clone().find_map(|i| graph.data(i)) {
            Some(d) => View::words(self.table(d)),
            None => self.none(),
        };
        let node = graph.nodes[graph.groups[job.from].nodes.start].id;
        let lc = match &local {
            Some(prog) => {
                let (b, warm, kept) = self.program_scratch(node, prog, job.keep);
                let tables = [&st, &dtv];
                // A program whose scratch depends on its input and itself alone, kept
                // from a render of the same input by the same program: nothing to run.
                let made = prog
                    .persist
                    .as_ref()
                    .filter(|p| p.reuse && kept)
                    .map(|_| prog.hash());
                let same = warm && made.is_some() && self.made_by(&b) == made;
                if !same {
                    if let Some(p) = prog.persist.as_ref().filter(|_| !warm) {
                        let reset = Program {
                            words: prog.words,
                            steps: p.reset.clone(),
                            persist: None,
                        };
                        let (input, img, region) = (job.input, job.img, job.region.input);
                        self.plan_program(rec, &reset, input, img, region, &b, tables)?;
                    }
                    let (input, img, region) = (job.input, job.img, job.region.input);
                    self.plan_program(rec, prog, input, img, region, &b, tables)?;
                }
                if kept && let Some(e) = self.kept.iter_mut().find(|e| e.1.id == b.id) {
                    e.2 = made;
                }
                Some((b, kept))
            }
            None => None,
        };
        let code = wgsl::run_source(graph, run, job.input.precision, job.output.precision);
        let nodes = graph.groups[job.from].nodes.start..graph.groups[g].nodes.end;
        let label = graph.nodes[nodes]
            .iter()
            .map(|nd| nd.op.name())
            .collect::<Vec<_>>()
            .join("+");
        let lcv = lc
            .as_ref()
            .map_or_else(|| self.none(), |(b, _)| View::words(b.clone()));
        let out = job.region.output;
        let w = rec.push(Planned {
            code,
            views: [job.output.clone(), job.input.clone(), st, lcv, dtv],
            threads: [out.w, out.h],
            label,
            slot: 0,
        });
        header(
            w,
            out,
            job.img,
            job.input,
            Some(job.output),
            job.region.input,
            job.params,
        );
        if let Some((b, false)) = lc {
            self.pool.release(b);
        }
        Ok(())
    }

    /// The scratch of `prog` on `node`: (buffer, whether it holds the last render's words
    /// for the same input, whether the engine keeps it). Persistent programs of interactive
    /// renders (`keep`, the input's identity) keep theirs; others take one from the pool.
    fn program_scratch(
        &mut self,
        node: (ModuleKind, u16),
        prog: &Program,
        keep: Option<Key>,
    ) -> (GpuBuf, bool, bool) {
        let Some(input) = keep.filter(|_| prog.persist.is_some()) else {
            let b = self
                .pool
                .acquire(&self.device, &mut self.next_id, prog.words * 4);
            return (b, false, false);
        };
        let k = KeptKey {
            node,
            input,
            words: prog.words,
        };
        if let Some(i) = self.kept.iter().position(|(kk, _, _)| *kk == k) {
            let e = self.kept.remove(i);
            let b = e.1.clone();
            self.kept.push(e);
            return (b, true, true);
        }
        // The node's scratch for another input or size (same region) is stale now. A
        // reusable program keeps its other sizes for this input (a draft and the refined
        // render of a node whose blend is being changed).
        let reuse = prog.persist.as_ref().is_some_and(|p| p.reuse);
        let stale: Vec<usize> = (0..self.kept.len())
            .filter(|&i| {
                let kk = &self.kept[i].0;
                kk.node == node && kk.input.roi == input.roi && (!reuse || kk.input != input)
            })
            .collect();
        for i in stale.into_iter().rev() {
            let (_, b, _) = self.kept.remove(i);
            self.pool.release(b);
        }
        let b = self
            .pool
            .acquire(&self.device, &mut self.next_id, prog.words * 4);
        self.kept.push((k, b.clone(), None));
        while self.kept.len() > KEPT {
            let (_, old, _) = self.kept.remove(0);
            self.pool.release(old);
        }
        (b, false, true)
    }

    /// The hash of the program that last wrote kept scratch `b`, when it may be reused.
    fn made_by(&self, b: &GpuBuf) -> Option<Hash> {
        self.kept.iter().find(|e| e.1.id == b.id).and_then(|e| e.2)
    }

    /// Plans the steps of `prog` over `region` of `input` (an `img`-sized image) into
    /// `scratch`.
    #[allow(clippy::too_many_arguments)]
    fn plan_program(
        &mut self,
        rec: &mut Rec,
        prog: &Program,
        input: &View,
        img: (u32, u32),
        region: Roi,
        scratch: &GpuBuf,
        [st, dt]: [&View; 2],
    ) -> Result<(), Error> {
        for step in &prog.steps {
            if step.words.len() > (PARAM_WORDS - HEADER_WORDS) as usize {
                return Err(Error::BadRequest("a program step has too many words"));
            }
            let code = wgsl::step_source(step.kernel, input.precision);
            let [tx, ty] = step.threads;
            let w = rec.push(Planned {
                code,
                views: [
                    View::words(scratch.clone()),
                    input.clone(),
                    st.clone(),
                    self.none(),
                    dt.clone(),
                ],
                threads: step.threads,
                label: step.kernel.name.to_owned(),
                slot: 0,
            });
            header(
                w,
                Roi::new(0, 0, tx, ty),
                img,
                input,
                None,
                region,
                &step.words,
            );
        }
        Ok(())
    }

    /// The statistics of every group whose first node takes them (from the cache, or
    /// planned now), on the grid of the whole source.
    #[allow(clippy::needless_range_loop)] // groups are indexed in several vectors
    fn statistics(
        &mut self,
        graph: &Graph,
        src: &Source,
        scale: f32,
        rec: &mut Rec,
    ) -> Result<Vec<Option<GpuBuf>>, Error> {
        let n = graph.groups.len();
        let mut out: Vec<Option<GpuBuf>> = vec![None; n];
        let (gw, gh) = stats_grid_at(src.width, src.height, self.stats_side);
        let gscale = scale * gw as f32 / src.width.max(1) as f32;
        // What each node's statistics are, and whether the cache has them.
        let mut wanted: Vec<(usize, StatsSlot, (Hash, Hash), Program)> = Vec::new();
        for g in 0..n {
            let Some(node) = graph.stats_node(g) else {
                continue;
            };
            let Some(prog) = node.op.stats(&node.params, gw, gh, gscale) else {
                continue;
            };
            let slot = (node.id, src.id(), gw, gh);
            let key = (graph.input_hash(g), prog.hash());
            match self.stats.get(&slot) {
                Some(e) if e.key == key => out[g] = Some(e.buf.clone()),
                _ => wanted.push((g, slot, key, prog)),
            }
        }
        let Some(&(last, ..)) = wanted.last() else {
            return Ok(out);
        };
        // The grid: the whole source, area-reduced.
        let whole = Roi::full(src.width, src.height);
        let sv = View::rows(
            src.buf.clone(),
            whole,
            0,
            src.height,
            src.precision,
            self.offset_align,
        );
        // The grid of the source is kept until the source changes.
        let none = self.none();
        let gbuf = match &self.grid {
            Some((id, w, h, b)) if (*id, *w, *h) == (src.id(), gw, gh) => b.clone(),
            _ => {
                if let Some((.., old)) = self.grid.take() {
                    self.pool.release(old);
                }
                let b = self.pool.acquire(
                    &self.device,
                    &mut self.next_id,
                    u64::from(gw) * u64::from(gh) * 16,
                );
                let mut grid = Program::default();
                // Per workgroup when a row of its cells fits (the same grid).
                let wg = wgsl::grid_reduce_wg_fits(src.width, gw);
                #[cfg(test)]
                let wg = wg && !PLAIN_GRID.load(std::sync::atomic::Ordering::Relaxed);
                if wg {
                    grid.step(
                        &wgsl::GRID_REDUCE_WG,
                        [gw.next_multiple_of(wgsl::WG), gh.next_multiple_of(wgsl::WG)],
                        vec![gw, gh],
                    );
                } else {
                    grid.step(&wgsl::GRID_REDUCE, [gw, gh], Vec::new());
                }
                self.plan_program(
                    rec,
                    &grid,
                    &sv,
                    (src.width, src.height),
                    whole,
                    &b,
                    [&none, &none],
                )?;
                self.grid = Some((src.id(), gw, gh, b.clone()));
                b
            }
        };
        let grid_id = gbuf.id;
        let groi = Roi::full(gw, gh);
        let mut cur = View::packed(gbuf, groi, Precision::F32);
        let mut want = wanted.into_iter().peekable();
        for g in 0..=last {
            if let Some((_, slot, key, prog)) = want.next_if(|w| w.0 == g) {
                let bytes = (prog.words * 4).max(16);
                let buf = match self.stats.remove(&slot) {
                    Some(old) if old.buf.size == pool::size_class(bytes) => old.buf,
                    Some(old) => {
                        self.pool.release(old.buf);
                        self.pool.acquire(&self.device, &mut self.next_id, bytes)
                    }
                    None => self.pool.acquire(&self.device, &mut self.next_id, bytes),
                };
                self.plan_program(rec, &prog, &cur, (gw, gh), groi, &buf, [&none, &none])?;
                self.keep_stats(slot, key, buf.clone());
                out[g] = Some(buf);
            }
            if g == last {
                break;
            }
            let d = View::packed(
                self.pool.acquire(
                    &self.device,
                    &mut self.next_id,
                    groi.pixels() * Precision::F32.bytes_per_pixel(),
                ),
                groi,
                Precision::F32,
            );
            let params = graph.params(g, gscale);
            let job = GroupJob {
                g,
                from: g,
                params: &params,
                input: &cur,
                output: &d,
                img: (gw, gh),
                region: crate::graph::GroupRoi {
                    input: groi,
                    output: groi,
                },
                scale: gscale,
                stats: out[g].as_ref(),
                keep: None,
            };
            self.plan_group(rec, graph, &job)?;
            let done = std::mem::replace(&mut cur, d).buf;
            if done.id != grid_id {
                self.pool.release(done);
            }
        }
        if cur.buf.id != grid_id {
            self.pool.release(cur.buf);
        }
        Ok(out)
    }

    /// Keeps statistics; drops those of other sources when many pile up.
    fn keep_stats(&mut self, slot: StatsSlot, key: (Hash, Hash), buf: GpuBuf) {
        if self.stats.len() >= 16 {
            let source = slot.1;
            let old: Vec<StatsSlot> = self
                .stats
                .keys()
                .filter(|k| k.1 != source)
                .copied()
                .collect();
            for k in old {
                if let Some(e) = self.stats.remove(&k) {
                    self.pool.release(e.buf);
                }
            }
        }
        self.stats.insert(slot, Stats { key, buf });
    }

    /// Pipelines and bind groups for the planned dispatches.
    fn materialize(&mut self, rec: &Rec) -> Result<Vec<Dispatch>, Error> {
        self.ensure_uniforms((rec.uniforms.len() / PARAM_WORDS as usize) as u64);
        let mut out = Vec::with_capacity(rec.planned.len());
        for p in &rec.planned {
            out.push(self.dispatch(p.slot, p)?);
        }
        Ok(out)
    }

    /// Makes sure the uniform buffer has `slots` dispatch blocks.
    fn ensure_uniforms(&mut self, slots: u64) {
        if self.uniforms.as_ref().is_some_and(|(_, s)| *s >= slots) {
            return;
        }
        let slots = slots.next_power_of_two().max(16);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop params"),
            size: slots * PARAM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.uniforms = Some((buffer, slots));
        // Every cached bind group bound the old buffer.
        self.bind_groups.clear();
    }

    fn pipeline(&mut self, code: &str) -> Result<wgpu::ComputePipeline, Error> {
        if let Some(p) = self.pipelines.get(code) {
            return Ok(p.clone());
        }
        let device = &self.device;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop kernel"),
            source: wgpu::ShaderSource::Wgsl(code.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("develop kernel"),
            layout: Some(&self.pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(Error::Gpu(format!("kernel does not build: {e}")));
        }
        self.counters.pipelines += 1;
        self.pipelines.insert(code.to_owned(), pipeline.clone());
        Ok(pipeline)
    }

    fn dispatch(&mut self, slot: u64, p: &Planned) -> Result<Dispatch, Error> {
        let pipeline = self.pipeline(&p.code)?;
        let gx = p.threads[0].div_ceil(wgsl::WG);
        let gy = p.threads[1].div_ceil(wgsl::WG);
        if gx > self.max_groups || gy > self.max_groups {
            return Err(Error::BadRequest("the region is too wide for one dispatch"));
        }
        let mk = self.masks.as_ref().map_or(&self.dummy, |m| &m.buf).clone();
        let k: BindKey = (slot, p.views.clone().map(|v| v.key()), mk.id);
        let bind = match self.bind_groups.get(&k) {
            Some(b) => b.clone(),
            None => {
                let (u, _) = self.uniforms.as_ref().expect("uniforms made");
                fn range(v: &View) -> wgpu::BindingResource<'_> {
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &v.buf.buffer,
                        offset: v.offset,
                        size: std::num::NonZeroU64::new(v.size),
                    })
                }
                let mut entries = vec![wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: u,
                        offset: slot * PARAM_BYTES,
                        size: std::num::NonZeroU64::new(PARAM_BYTES),
                    }),
                }];
                for (i, v) in p.views.iter().enumerate() {
                    entries.push(wgpu::BindGroupEntry {
                        binding: i as u32 + 1,
                        resource: range(v),
                    });
                }
                entries.push(wgpu::BindGroupEntry {
                    binding: 6,
                    resource: mk.buffer.as_entire_binding(),
                });
                let b = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("develop dispatch"),
                    layout: &self.layout,
                    entries: &entries,
                });
                self.counters.bind_groups += 1;
                self.bind_groups.insert(k, b.clone());
                b
            }
        };
        Ok(Dispatch {
            pipeline,
            bind,
            groups: [gx, gy],
            label: p.label.clone(),
            threads: u64::from(p.threads[0]) * u64::from(p.threads[1]),
        })
    }

    /// Reads a render back as f32 RGBA (f16 storage is widened).
    pub fn read(&self, r: &Rendered) -> Result<Vec<f32>, Error> {
        if self.device_lost() {
            return Err(Error::DeviceLost);
        }
        let bytes = r.bytes();
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop read"),
            size: bytes.div_ceil(4) * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("develop read"),
            });
        enc.copy_buffer_to_buffer(&r.buf.buffer, 0, &staging, 0, bytes);
        self.queue.submit([enc.finish()]);
        let out = {
            let view = self.or_lost(map_read(&self.device, &staging, bytes))?;
            match r.precision {
                Precision::F32 => bytemuck::pod_collect_to_vec::<u8, f32>(&view[..]),
                Precision::F16 => bytemuck::pod_collect_to_vec::<u8, u16>(&view[..])
                    .into_iter()
                    .map(|h| half::f16::from_bits(h).to_f32())
                    .collect(),
            }
        };
        staging.unmap();
        Ok(out)
    }

    /// Whether rendering `graph` on `src` as `req` asks would copy a kept result.
    pub fn has_result(&self, graph: &Graph, src: &Source, req: &Request) -> bool {
        let n = graph.groups.len();
        req.checkpoints
            && req.keep_result
            && n > 0
            && self.cache.contains(&Key {
                prefix: graph.group_hash(n - 1),
                source: src.id(),
                roi: req.roi,
                scale_bits: req.scale.to_bits(),
                precision: req.precision,
                version: GRAPH_VERSION ^ 0x8000_0000,
            })
    }

    /// The first group rendering `graph` on `src` as `req` asks would run (the one after
    /// the latest checkpoint kept for the whole region; `0`: from the source). Looks only;
    /// a render that tiles may resume elsewhere per tile.
    pub fn resumes_at(&self, graph: &Graph, src: &Source, req: &Request) -> usize {
        let n = graph.groups.len();
        if !req.checkpoints {
            return 0;
        }
        let rois = graph.rois(req.roi, src.width, src.height, req.scale);
        (0..n.saturating_sub(1))
            .rev()
            .find(|&g| {
                (graph.groups[g].checkpoint || req.checkpoint_every)
                    && self.cache.contains(&Key {
                        prefix: graph.group_hash(g),
                        source: src.id(),
                        roi: rois[g].output,
                        scale_bits: req.scale.to_bits(),
                        precision: req.precision,
                        version: GRAPH_VERSION,
                    })
            })
            .map_or(0, |g| g + 1)
    }

    /// The statistics grid kept for the last source (tests): its size and RGBA values.
    #[cfg(test)]
    pub(crate) fn grid_values(&self) -> Option<(u32, u32, Vec<f32>)> {
        let (_, w, h, b) = self.grid.as_ref()?;
        let bytes = u64::from(*w) * u64::from(*h) * 16;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop grid read"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("develop grid read"),
            });
        enc.copy_buffer_to_buffer(&b.buffer, 0, &staging, 0, bytes);
        self.queue.submit([enc.finish()]);
        let v = {
            let view = map_read(&self.device, &staging, bytes).ok()?;
            bytemuck::pod_collect_to_vec::<u8, f32>(&view[..])
        };
        staging.unmap();
        Some((*w, *h, v))
    }

    /// Waits for every submitted render (timing).
    pub fn wait(&self) -> Result<(), Error> {
        if self.device_lost() {
            return Err(Error::DeviceLost);
        }
        let r = poll(&self.device, wgpu::PollType::wait_indefinitely());
        self.or_lost(r)
    }
}

/// What [`Engine::plan_group`] plans.
struct GroupJob<'a> {
    g: usize,
    /// The first group of a run of point groups planned as one kernel ending at `g`
    /// ([`Engine::run_end`]); `g` itself otherwise.
    from: usize,
    /// The params of the groups `from..=g`, in order.
    params: &'a [u32],
    input: &'a View,
    output: &'a View,
    /// The image size (sampling clamps to it).
    img: (u32, u32),
    region: crate::graph::GroupRoi,
    scale: f32,
    stats: Option<&'a GpuBuf>,
    /// The input's identity when a persistent local program may keep its scratch.
    keep: Option<Key>,
}

/// Writes a dispatch's header and params into its uniform block `w`.
fn header(
    w: &mut [u32],
    region: Roi,
    img: (u32, u32),
    s: &View,
    d: Option<&View>,
    prog: Roi,
    params: &[u32],
) {
    w[..4].copy_from_slice(&[region.x, region.y, region.w, region.h]);
    w[4] = img.0;
    w[5] = img.1;
    w[6..10].copy_from_slice(&s.header());
    if let Some(d) = d {
        w[10..14].copy_from_slice(&d.header());
    }
    w[16..20].copy_from_slice(&[prog.x, prog.y, prog.w, prog.h]);
    let h = HEADER_WORDS as usize;
    w[h..h + params.len()].copy_from_slice(params);
}

/// Records the dispatches: one compute pass, or one pass each with timestamps around it.
/// `timestamps` is the query set and the index of `ds[0]` among the render's dispatches.
fn record(
    enc: &mut wgpu::CommandEncoder,
    ds: &[Dispatch],
    timestamps: Option<(&wgpu::QuerySet, usize)>,
) {
    fn go(cp: &mut wgpu::ComputePass<'_>, d: &Dispatch) {
        cp.set_pipeline(&d.pipeline);
        cp.set_bind_group(0, &d.bind, &[]);
        cp.dispatch_workgroups(d.groups[0], d.groups[1], 1);
    }
    match timestamps {
        None => {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("develop render"),
                timestamp_writes: None,
            });
            for d in ds {
                go(&mut cp, d);
            }
        }
        Some((qs, first)) => {
            for (i, d) in ds.iter().enumerate() {
                let i = first + i;
                let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("develop profile"),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                        query_set: qs,
                        beginning_of_pass_write_index: Some(2 * i as u32),
                        end_of_pass_write_index: Some(2 * i as u32 + 1),
                    }),
                });
                go(&mut cp, d);
            }
        }
    }
}

/// Estimated GPU milliseconds one queue submission may hold. Drivers reset a device whose
/// submission runs for seconds (amdgpu and i915 after 2–10 s, Windows TDR after 2 s), and
/// sooner when other work shares the GPU; small submissions also let a compositor's frames
/// in between.
pub const SUBMIT_MS: f64 = 150.0;

/// GPU nanoseconds per thread assumed for a kernel not measured yet: about the slowest
/// diffuse band step on an integrated GPU (measured 2026-11-01 on a Radeon 860M).
const DEFAULT_NS_PER_THREAD: f64 = 8.0;

fn estimate(speeds: &HashMap<String, f64>, label: &str, threads: u64) -> f64 {
    let ns = speeds.get(label).copied().unwrap_or(DEFAULT_NS_PER_THREAD);
    ns * threads as f64 * 1e-6
}

/// Takes a profiled dispatch's time into its kernel's estimate: a slower reading at once
/// (keeps submissions short when the GPU is busy), a faster one slowly.
fn learn(speeds: &mut HashMap<String, f64>, d: &Dispatch, ms: f64) {
    if d.threads < 4096 || !ms.is_finite() || ms <= 0.0 {
        return;
    }
    let ns = ms * 1e6 / d.threads as f64;
    let e = speeds.entry(d.label.clone()).or_insert(ns);
    *e = if ns > *e { ns } else { 0.875 * *e + 0.125 * ns };
}

/// Splits `ds` into runs of at most `limit` estimated milliseconds (a dispatch over the
/// limit alone), in order.
fn submissions(
    ds: &[Dispatch],
    speeds: &HashMap<String, f64>,
    limit: f64,
) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let (mut start, mut sum) = (0, 0.0);
    for (i, d) in ds.iter().enumerate() {
        let t = estimate(speeds, &d.label, d.threads);
        if i > start && sum + t > limit {
            out.push(start..i);
            start = i;
            sum = 0.0;
        }
        sum += t;
    }
    if start < ds.len() {
        out.push(start..ds.len());
    }
    out
}

/// Polls `device`. wgpu panics in a poll that finds the device lost (a driver reset under
/// a long submission, seen at 24 MP under load): that is caught and reported as
/// [`Error::DeviceLost`], so the app can open a new device instead of going down.
pub fn poll(device: &wgpu::Device, ty: wgpu::PollType) -> Result<(), Error> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| device.poll(ty))) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(Error::Gpu(e.to_string())),
        Err(_) => Err(Error::DeviceLost),
    }
}

/// The last group of the run starting at `from` that one kernel computes (`from`
/// itself when `on` is false or the next group cannot join). A run starts at a fused point
/// group: one starting at a kept local program (local contrast) cost a colour balance or
/// curves slider its resume point after local contrast (3.3 → 8.6 ms GPU). A group joins it when it is a fused point group over the region
/// the one before puts out, the one before is no checkpoint (a heavy op's output, or the
/// module in focus), and the run still binds at most one node's statistics and one table,
/// and fits one uniform block. The outputs left out are cheap to make again: resuming
/// before them reads the same picture their kernel would.
fn run_end(
    graph: &Graph,
    rois: &[crate::graph::GroupRoi],
    params: &[Vec<u32>],
    from: usize,
    on: bool,
) -> usize {
    let point = |g: usize| graph.groups[g].fused && rois[g].input == rois[g].output;
    if !on || !point(from) {
        return from;
    }
    let mut stats = usize::from(graph.stats_node(from).is_some());
    let mut tables = usize::from(graph.data(from).is_some());
    let mut words = params[from].len();
    let mut e = from;
    while e + 1 < graph.groups.len() {
        let next = e + 1;
        let s = stats + usize::from(graph.stats_node(next).is_some());
        let t = tables + usize::from(graph.data(next).is_some());
        let w = words + params[next].len();
        if graph.groups[e].checkpoint
            || !point(next)
            || rois[e].output != rois[next].input
            || s > 1
            || t > 1
            || w > (PARAM_WORDS - HEADER_WORDS) as usize
        {
            break;
        }
        (stats, tables, words, e) = (s, t, w, next);
    }
    e
}

/// Tests reduce the statistics grid per thread ([`wgsl::GRID_REDUCE`]) only.
#[cfg(test)]
pub(crate) static PLAIN_GRID: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Whether a wgpu error message says the device is gone.
fn lost_message(m: &str) -> bool {
    let m = m.to_ascii_lowercase();
    m.contains("device is lost") || m.contains("device lost") || m.contains("devicelost")
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

/// Maps `bytes` of `buffer` for reading and waits for it.
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
    poll(device, wgpu::PollType::wait_indefinitely())?;
    // Another thread polling the device may hold the callback: wait for it.
    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(true) => {}
        _ => return Err(Error::Gpu("reading the result back failed".into())),
    }
    buffer
        .slice(..bytes)
        .get_mapped_range()
        .map_err(|e| Error::Gpu(format!("{e:?}")))
}
