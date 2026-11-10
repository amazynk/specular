//! Inspection passes (DESIGN.md §6.2): the clipping and focus-peaking overlay, the histogram
//! of the visible image and the pixel probe.
//!
//! All of them bind the image exactly as [`ImageRenderer`] draws it (`bind_image`), so they
//! see the same resident tiles and overview.
//!
//! - The overlay is one fullscreen pass blended over the scene; it costs nothing when off.
//! - The histogram renders the visible part of the image into a target of at most
//!   [`HIST_SIZE`]² (one texel of the finest fitting level per target pixel), bins it in a
//!   compute pass, and reads back 4 KiB.
//! - The probe reads the texel under the pointer (and which level it came from).
//!
//! Readbacks never stall a frame: [`Inspector::request`] records the work into the frame's
//! encoder, [`Inspector::after_submit`] starts mapping once it is submitted, and
//! [`Inspector::poll`] picks the result up on a later frame. One request is in flight at a
//! time; while one is, new requests are refused (the caller asks again next frame).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bytemuck::{Pod, Zeroable};
use specular_gfx::BufferInit;
use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, wgpu};
use specular_inspect::{DisplayValue, Histogram};

use super::{GpuImage, ImageRenderer, RenderOptions};
use crate::view::{PxRect, View};

/// Largest side of the histogram sample target.
pub const HIST_SIZE: u32 = 256;

const PROBE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const HIST_BYTES: u64 = Histogram::WORDS as u64 * 4;
/// Offset of the probe texels in the readback buffer (texture copies want 256-byte rows).
const PROBE_OFFSET: u64 = HIST_BYTES.div_ceil(256) * 256;
const READBACK_BYTES: u64 = PROBE_OFFSET + 256;

/// The shader source of the fragment passes: the image shader plus `inspect.wgsl`.
pub(crate) fn overlay_source() -> String {
    format!(
        "{FULLSCREEN_WGSL}\n{}\n{}",
        include_str!("../shaders/image.wgsl"),
        include_str!("../shaders/inspect.wgsl")
    )
}

pub(crate) const HISTOGRAM_WGSL: &str = include_str!("../shaders/histogram.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InspUniform {
    a: [f32; 4],
    peak: [f32; 4],
    hi: [f32; 4],
    lo: [f32; 4],
    b: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HistParams {
    size: [u32; 2],
    low: u32,
    high: u32,
}

/// What the overlay pass draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayOptions {
    pub clipping: bool,
    pub peaking: bool,
    /// Shadow clipping threshold: a pixel with any channel at or below this sRGB code.
    pub clip_low: u8,
    /// Highlight clipping threshold: a pixel with any channel at or above this sRGB code.
    pub clip_high: u8,
    /// Sobel magnitude an edge needs (`specular_inspect::peaking_threshold`).
    pub peaking_threshold: f32,
    /// Linear, premultiplied colours.
    pub peaking_color: [f32; 4],
    pub highlight_color: [f32; 4],
    pub shadow_color: [f32; 4],
}

impl Default for OverlayOptions {
    fn default() -> Self {
        OverlayOptions {
            clipping: false,
            peaking: false,
            clip_low: specular_inspect::DEFAULT_CLIP_LOW,
            clip_high: specular_inspect::DEFAULT_CLIP_HIGH,
            peaking_threshold: specular_inspect::peaking_threshold(50),
            peaking_color: [1.0, 0.0, 1.0, 1.0],
            highlight_color: [1.0, 0.0, 0.0, 1.0],
            shadow_color: [0.0, 0.2, 1.0, 1.0],
        }
    }
}

impl OverlayOptions {
    pub fn active(&self) -> bool {
        self.clipping || self.peaking
    }

    fn uniform(&self, probe: (f32, f32)) -> InspUniform {
        let on = |b: bool| if b { 1.0 } else { 0.0 };
        InspUniform {
            a: [
                on(self.clipping),
                on(self.peaking),
                self.clip_high as f32,
                self.clip_low as f32,
            ],
            peak: self.peaking_color,
            hi: self.highlight_color,
            lo: self.shadow_color,
            b: [self.peaking_threshold, 1.0, probe.0, probe.1],
        }
    }
}

/// A histogram request: the clipping thresholds the clipped counts use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistogramRequest {
    pub clip_low: u8,
    pub clip_high: u8,
}

/// A finished readback.
#[derive(Clone, Debug, Default)]
pub struct Readback {
    /// `None` when no histogram was asked for (an empty one when nothing was visible).
    pub histogram: Option<Histogram>,
    /// `None` when no probe was asked for or nothing is resident there.
    pub probe: Option<DisplayValue>,
    /// Milliseconds from the request being recorded to the result being read (GPU latency
    /// across frames, not frame time).
    pub latency_ms: f64,
}

enum State {
    Idle,
    /// Recorded into an encoder that is not submitted yet.
    Recorded {
        hist: bool,
        probe: bool,
        at: std::time::Instant,
    },
    /// Submitted; mapping (0 pending, 1 mapped, 2 failed).
    Mapping {
        hist: bool,
        probe: bool,
        at: std::time::Instant,
        done: Arc<AtomicU8>,
    },
}

pub struct Inspector {
    overlay_pipe: wgpu::RenderPipeline,
    sample_pipe: wgpu::RenderPipeline,
    probe_pipe: wgpu::RenderPipeline,
    hist_pipe: wgpu::ComputePipeline,
    insp_bgl: wgpu::BindGroupLayout,
    hist_bgl: wgpu::BindGroupLayout,
    sample_view: wgpu::TextureView,
    probe_tex: wgpu::Texture,
    probe_view: wgpu::TextureView,
    bins: wgpu::Buffer,
    readback: wgpu::Buffer,
    state: State,
    /// CPU milliseconds the last `request` spent recording.
    pub last_record_ms: f64,
}

impl std::fmt::Debug for Inspector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inspector")
            .field("busy", &self.busy())
            .finish_non_exhaustive()
    }
}

/// An [`Inspector`] being built on a background thread: compiling its pipelines takes tens of
/// milliseconds, which would otherwise stall the frame that switches the first tool on.
pub struct InspectorBuild {
    rx: std::sync::mpsc::Receiver<Inspector>,
    ready: Option<Inspector>,
}

impl std::fmt::Debug for InspectorBuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InspectorBuild")
            .field("ready", &self.ready.is_some())
            .finish_non_exhaustive()
    }
}

impl InspectorBuild {
    /// The inspector, once built (never blocks).
    pub fn try_take(&mut self) -> Option<Inspector> {
        self.ready.take().or_else(|| self.rx.try_recv().ok())
    }

    /// Whether the build ended without an inspector (the thread panicked).
    pub fn failed(&self) -> bool {
        self.ready.is_none()
            && matches!(
                self.rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Disconnected)
            )
    }

    /// Waits for the inspector (tests).
    pub fn wait(mut self) -> Option<Inspector> {
        self.ready.take().or_else(|| self.rx.recv().ok())
    }
}

impl Inspector {
    /// Builds an inspector on a background thread; `done` runs on that thread once it is
    /// ready (to wake the event loop). Falls back to building here when no thread can start.
    pub fn spawn(
        gpu: &Gpu,
        renderer: &ImageRenderer,
        done: impl FnOnce() + Send + 'static,
    ) -> InspectorBuild {
        let (tx, rx) = std::sync::mpsc::channel();
        let g = gpu.clone();
        let bgl = renderer.image_bgl.clone();
        let spawned = specular_thread::builder("gpu-inspect").spawn(move || {
            let _ = tx.send(Inspector::with_layout(&g, &bgl));
            done();
        });
        let ready = spawned
            .is_err()
            .then(|| Inspector::with_layout(gpu, &renderer.image_bgl));
        InspectorBuild { rx, ready }
    }

    pub fn new(gpu: &Gpu, renderer: &ImageRenderer) -> Inspector {
        Inspector::with_layout(gpu, &renderer.image_bgl)
    }

    /// Builds the pipelines; `image_bgl` is the renderer's image bind group layout.
    fn with_layout(gpu: &Gpu, image_bgl: &wgpu::BindGroupLayout) -> Inspector {
        let t0 = std::time::Instant::now();
        let d = &gpu.device;
        let module = specular_gfx::shader(d, "inspect", &overlay_source());
        let insp_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("inspect"),
            entries: &[super::uniform_entry(0)],
        });
        let overlay_pipe = fullscreen_pipeline(
            gpu,
            "inspect overlay",
            &module,
            "fs_overlay",
            &[Some(image_bgl), Some(&insp_bgl)],
            HDR,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        let sample_pipe = fullscreen_pipeline(
            gpu,
            "histogram sample",
            &module,
            "fs_sample",
            &[Some(image_bgl)],
            HDR,
            None,
        );
        let probe_pipe = fullscreen_pipeline(
            gpu,
            "pixel probe",
            &module,
            "fs_probe",
            &[Some(image_bgl), Some(&insp_bgl)],
            PROBE_FORMAT,
            None,
        );
        let hist_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("histogram"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let hmod = specular_gfx::shader(d, "histogram", HISTOGRAM_WGSL);
        let hist_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("histogram"),
            bind_group_layouts: &[Some(&hist_bgl)],
            immediate_size: 0,
        });
        let hist_pipe = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("histogram"),
            layout: Some(&hist_layout),
            module: &hmod,
            entry_point: Some("cs_histogram"),
            compilation_options: Default::default(),
            cache: gpu.cache(),
        });
        let sample_view = super::texture_2d(
            d,
            "histogram sample",
            HIST_SIZE,
            HIST_SIZE,
            1,
            1,
            HDR,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        )
        .create_view(&Default::default());
        let probe_tex = super::texture_2d(
            d,
            "pixel probe",
            2,
            1,
            1,
            1,
            PROBE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let probe_view = probe_tex.create_view(&Default::default());
        let bins = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram bins"),
            size: HIST_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let readback = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("inspect readback"),
            size: READBACK_BYTES,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        specular_log::debug!(target: specular_log::RENDER, "inspect: pipelines built in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
        Inspector {
            overlay_pipe,
            sample_pipe,
            probe_pipe,
            hist_pipe,
            insp_bgl,
            hist_bgl,
            sample_view,
            probe_tex,
            probe_view,
            bins,
            readback,
            state: State::Idle,
            last_record_ms: 0.0,
        }
    }

    fn insp_bind(&self, gpu: &Gpu, u: &InspUniform) -> wgpu::BindGroup {
        let ub = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("inspect"),
            contents: bytemuck::bytes_of(u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("inspect"),
            layout: &self.insp_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ub.as_entire_binding(),
            }],
        })
    }

    /// Draws the clipping / peaking overlay for `view` over `target` (the scene, already
    /// holding the image drawn with the same `view` and `opts`). Does nothing when `o` has
    /// nothing switched on or nothing of the image is resident.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_overlay(
        &self,
        gpu: &Gpu,
        renderer: &ImageRenderer,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        img: &GpuImage,
        view: &View,
        opts: &RenderOptions,
        o: &OverlayOptions,
    ) {
        if !o.active() || !resident(renderer, img) {
            return;
        }
        let vp = view.viewport;
        let x0 = vp.x.max(0.0).floor() as u32;
        let y0 = vp.y.max(0.0).floor() as u32;
        let x1 = ((vp.x + vp.w).ceil().max(0.0) as u32).min(size.0);
        let y1 = ((vp.y + vp.h).ceil().max(0.0) as u32).min(size.1);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let image = renderer.bind_image(gpu, img, view, opts, None);
        let insp = self.insp_bind(gpu, &o.uniform((0.0, 0.0)));
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("inspect overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        rp.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
        rp.set_pipeline(&self.overlay_pipe);
        rp.set_bind_group(0, &image, &[]);
        rp.set_bind_group(1, &insp, &[]);
        rp.draw(0..3, 0..1);
    }

    /// A request is recorded or in flight.
    pub fn busy(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Records a histogram of what `view` shows and/or a probe of stored pixel `probe` into
    /// `enc`. Returns false (recording nothing) while an earlier request is in flight, or
    /// when there is nothing to do. Call [`Inspector::after_submit`] once `enc` is submitted.
    #[allow(clippy::too_many_arguments)]
    pub fn request(
        &mut self,
        gpu: &Gpu,
        renderer: &ImageRenderer,
        enc: &mut wgpu::CommandEncoder,
        img: &GpuImage,
        view: &View,
        hist: Option<HistogramRequest>,
        probe: Option<(u32, u32)>,
    ) -> bool {
        if self.busy() || (hist.is_none() && probe.is_none()) || !resident(renderer, img) {
            return false;
        }
        let t0 = std::time::Instant::now();
        let opts = RenderOptions {
            hq: false,
            // Nearest everywhere: one texel per sample, never a blend of neighbours.
            nearest_from: 0.0,
            grid: None,
            ..RenderOptions::default()
        };
        if let Some(h) = hist {
            enc.clear_buffer(&self.bins, 0, None);
            let (tw, th, sv) = sample_view(view, img);
            if tw > 0 {
                let image = renderer.bind_image(gpu, img, &sv, &opts, None);
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("histogram sample"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.sample_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                rp.set_scissor_rect(0, 0, tw, th);
                rp.set_pipeline(&self.sample_pipe);
                rp.set_bind_group(0, &image, &[]);
                rp.draw(0..3, 0..1);
                drop(rp);
                let params = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("histogram params"),
                    contents: bytemuck::bytes_of(&HistParams {
                        size: [tw, th],
                        low: h.clip_low as u32,
                        high: h.clip_high as u32,
                    }),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("histogram"),
                    layout: &self.hist_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.sample_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.bins.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: params.as_entire_binding(),
                        },
                    ],
                });
                let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("histogram"),
                    timestamp_writes: None,
                });
                cp.set_pipeline(&self.hist_pipe);
                cp.set_bind_group(0, &bind, &[]);
                cp.dispatch_workgroups(tw.div_ceil(16), th.div_ceil(16), 1);
            }
            enc.copy_buffer_to_buffer(&self.bins, 0, &self.readback, 0, HIST_BYTES);
        }
        if let Some(p) = probe {
            let p = (p.0.min(img.width - 1), p.1.min(img.height - 1));
            // The probe needs the image uniform only for the tile table and sizes; any
            // view of the image will do.
            let image = renderer.bind_image(gpu, img, view, &opts, None);
            let insp = self.insp_bind(
                gpu,
                &OverlayOptions::default().uniform((p.0 as f32, p.1 as f32)),
            );
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pixel probe"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.probe_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            rp.set_pipeline(&self.probe_pipe);
            rp.set_bind_group(0, &image, &[]);
            rp.set_bind_group(1, &insp, &[]);
            rp.draw(0..3, 0..1);
            drop(rp);
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.probe_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: PROBE_OFFSET,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 2,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.state = State::Recorded {
            hist: hist.is_some(),
            probe: probe.is_some(),
            at: t0,
        };
        self.last_record_ms = t0.elapsed().as_secs_f64() * 1e3;
        true
    }

    /// Starts mapping the readback once the encoder holding the request is submitted.
    pub fn after_submit(&mut self) {
        let State::Recorded { hist, probe, at } = self.state else {
            return;
        };
        let done = Arc::new(AtomicU8::new(0));
        let d = done.clone();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                d.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
        self.state = State::Mapping {
            hist,
            probe,
            at,
            done,
        };
    }

    /// Drops a recorded request whose encoder was never submitted.
    pub fn cancel_recorded(&mut self) {
        if matches!(self.state, State::Recorded { .. }) {
            self.state = State::Idle;
        }
    }

    /// The finished readback, if the GPU is done with it (never blocks).
    pub fn poll(&mut self, gpu: &Gpu) -> Option<Readback> {
        let State::Mapping {
            hist,
            probe,
            at,
            ref done,
        } = self.state
        else {
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
        let out = {
            let slice = self.readback.slice(..);
            let data = slice.get_mapped_range().ok()?;
            let words: &[u32] = bytemuck::cast_slice(&data[..HIST_BYTES as usize]);
            let histogram = if hist {
                Histogram::from_words(words)
            } else {
                None
            };
            let probe = probe
                .then(|| {
                    let px: &[f32] = bytemuck::cast_slice(
                        &data[PROBE_OFFSET as usize..PROBE_OFFSET as usize + 32],
                    );
                    let level = px[4];
                    (level >= 0.0).then(|| {
                        DisplayValue::from_premultiplied([px[0], px[1], px[2], px[3]], level as u32)
                    })
                })
                .flatten();
            Readback {
                histogram,
                probe,
                latency_ms: at.elapsed().as_secs_f64() * 1e3,
            }
        };
        self.readback.unmap();
        self.state = State::Idle;
        Some(out)
    }
}

/// Whether anything of `img` is on the GPU.
pub(crate) fn resident(renderer: &ImageRenderer, img: &GpuImage) -> bool {
    img.overview.is_some() || renderer.pool.tiles_of(img.id).next().is_some()
}

/// The view that draws what `view` shows of the image into the top-left `w × h` of the
/// histogram target, at no more than one target pixel per stored pixel.
fn sample_view(view: &View, img: &GpuImage) -> (u32, u32, View) {
    let (x0, y0, x1, y1) = view.visible_display();
    let (dw, dh) = (x1 - x0, y1 - y0);
    let fixed = |w: u32, h: u32, k: f32| {
        View::fixed(
            (img.width, img.height),
            view.orientation(),
            PxRect {
                x: 0.0,
                y: 0.0,
                w: w as f32,
                h: h as f32,
            },
            k,
            ((x0 + x1) * 0.5, (y0 + y1) * 0.5),
        )
    };
    if dw < 0.5 || dh < 0.5 {
        return (0, 0, fixed(1, 1, 1.0));
    }
    let k = (HIST_SIZE as f32 / dw.max(dh)).min(1.0);
    let w = ((dw * k).round() as u32).clamp(1, HIST_SIZE);
    let h = ((dh * k).round() as u32).clamp(1, HIST_SIZE);
    // Scale per axis may differ by rounding; the larger keeps every target pixel inside.
    let k = (w as f32 / dw).max(h as f32 / dh);
    (w, h, fixed(w, h, k))
}

#[cfg(test)]
mod tests;
