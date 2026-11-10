//! Gaussian splats on the GPU. A [`SplatsGpu`] holds a [`Splats`] in two storage buffers (48
//! bytes a splat: centre, covariance, base colour and opacity; then the higher harmonics as
//! half floats), uploaded at most [`UPLOAD_PER_FRAME`] bytes a frame. Files over the GPU's
//! buffer limit are evenly thinned (and their harmonics dropped first), with a note.
//!
//! [`SplatRenderer::draw`] sorts the splats in front of the eye by depth on the CPU whenever
//! the camera moved (a two-pass 16-bit radix sort, parallel key computation), uploads the
//! order and draws them. Past [`ASYNC_SORT_MIN`] splats the sort runs on a worker and frames
//! draw the last order until the new one lands ([`SplatsGpu::sorting`]); then the splats back to front as alpha-blended anisotropic Gaussians, the colour
//! evaluated along each splat's view ray, into a display-referred target that is then
//! composited over the linear scene, under the mesh view's camera and over its ground grid.

use std::sync::Arc;

use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, shader, wgpu};
use specular_splat::{Splats, f32_to_f16};

use crate::{Camera, PREMULTIPLIED, frame_for, layout_entry, storage, uniform};

/// Bytes of one splat's core record.
const RECORD: usize = 48;

/// Bytes uploaded a frame at most.
pub const SPLAT_UPLOAD_PER_FRAME: usize = 32 << 20;

/// Splats from which a new order is sorted on a worker, not in the frame (smaller sets sort
/// in a few milliseconds).
pub const ASYNC_SORT_MIN: usize = 400_000;

/// A sort running on a worker: the view it is for, and where its order and time arrive.
type PendingSort = ([f32; 7], std::sync::mpsc::Receiver<(Vec<u32>, f32)>);

/// One view of splats.
#[derive(Clone, Copy, Debug)]
pub struct SplatView {
    /// Target rectangle: x, y, width, height (physical px).
    pub rect: [u32; 4],
    pub camera: Camera,
    pub grid: bool,
}

/// Splats on the GPU.
pub struct SplatsGpu {
    splats: Arc<Splats>,
    /// Every `stride`th splat is on the GPU.
    stride: usize,
    /// Splats the GPU holds (when complete).
    n: usize,
    /// Harmonic coefficients per channel kept (0 when the buffer would be too large).
    k: usize,
    /// u32 words of harmonics a splat.
    sh_words: usize,
    core: wgpu::Buffer,
    sh: wgpu::Buffer,
    order: wgpu::Buffer,
    group: Option<wgpu::BindGroup>,
    /// Splats uploaded so far.
    uploaded: usize,
    /// The view the order was sorted for, and how many it holds.
    sorted_for: Option<[f32; 7]>,
    sorted: u32,
    /// A sort on a worker, when one runs.
    pending: Option<PendingSort>,
    /// The box the camera frames (the splats' robust bounds).
    pub bounds: specular_splat::Bounds,
    /// What was thinned or dropped to fit the GPU.
    pub notes: Vec<String>,
    /// Splats drawn by the last frame, and the time its sort took (ms; 0 when the order was
    /// reused).
    pub drawn: u32,
    pub sort_ms: f32,
}

impl SplatsGpu {
    /// Buffers for `splats` (empty until fed).
    pub fn new(gpu: &Gpu, splats: Arc<Splats>) -> SplatsGpu {
        let limit = gpu
            .device
            .limits()
            .max_storage_buffer_binding_size
            .min(gpu.device.limits().max_buffer_size) as usize;
        let total = splats.len();
        let mut notes = Vec::new();
        let stride = total.div_ceil((limit / RECORD).max(1)).max(1);
        let n = total.div_ceil(stride);
        if stride > 1 {
            notes.push(format!(
                "every {stride}th splat shown (the GPU's buffer limit)"
            ));
        }
        let mut k = splats.sh_coeffs();
        let words = |k: usize| (k * 3).div_ceil(2);
        while k > 0 && n * words(k) * 4 > limit {
            k = match k {
                15 => 8,
                8 => 3,
                _ => 0,
            };
        }
        if k < splats.sh_coeffs() {
            notes.push("view-dependent colour reduced to fit the GPU".to_owned());
        }
        let sh_words = words(k);
        let buf = |label, size: usize| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        SplatsGpu {
            core: buf("splats core", n * RECORD),
            sh: buf("splats harmonics", n * sh_words * 4),
            order: buf("splats order", n * 4),
            group: None,
            stride,
            n,
            k,
            sh_words,
            uploaded: 0,
            sorted_for: None,
            sorted: 0,
            pending: None,
            bounds: splats.bounds,
            notes,
            splats,
            drawn: 0,
            sort_ms: 0.0,
        }
    }

    pub fn len(&self) -> usize {
        self.uploaded
    }

    pub fn is_empty(&self) -> bool {
        self.uploaded == 0
    }

    pub fn complete(&self) -> bool {
        self.uploaded == self.n
    }

    /// A new order is being sorted on a worker: draw again to show it when it lands.
    pub fn sorting(&self) -> bool {
        self.pending.is_some()
    }

    /// Bytes on the GPU.
    pub fn bytes(&self) -> u64 {
        self.core.size() + self.sh.size() + self.order.size()
    }

    /// Uploads the next splats, at most [`SPLAT_UPLOAD_PER_FRAME`] bytes; true when what is
    /// drawn changed.
    pub fn feed(&mut self, gpu: &Gpu) -> bool {
        if self.complete() {
            return false;
        }
        let per = RECORD + self.sh_words * 4;
        let count = (SPLAT_UPLOAD_PER_FRAME / per)
            .max(1)
            .min(self.n - self.uploaded);
        let (a, b) = (self.uploaded, self.uploaded + count);
        let s = &self.splats;
        let mut core = Vec::with_capacity(count * RECORD);
        let mut sh = Vec::with_capacity(count * self.sh_words * 4);
        let all_k = s.sh_coeffs() * 3;
        for g in a..b {
            let i = g * self.stride;
            for v in s.positions[i].iter().chain(&s.covariances[i]) {
                core.extend_from_slice(&v.to_le_bytes());
            }
            let c = s.colors[i];
            for pair in [[c[0], c[1]], [c[2], c[3]]] {
                let w = u32::from(f32_to_f16(pair[0])) | u32::from(f32_to_f16(pair[1])) << 16;
                core.extend_from_slice(&w.to_le_bytes());
            }
            core.extend_from_slice(&[0; 4]);
            if self.sh_words > 0 {
                let src = &s.sh[i * all_k..i * all_k + self.k * 3];
                for w in 0..self.sh_words {
                    let lo = src.get(w * 2).copied().unwrap_or(0);
                    let hi = src.get(w * 2 + 1).copied().unwrap_or(0);
                    sh.extend_from_slice(&(u32::from(lo) | u32::from(hi) << 16).to_le_bytes());
                }
            }
        }
        gpu.queue
            .write_buffer(&self.core, (a * RECORD) as u64, &core);
        if !sh.is_empty() {
            gpu.queue
                .write_buffer(&self.sh, (a * self.sh_words * 4) as u64, &sh);
        }
        self.uploaded = b;
        self.sorted_for = None;
        true
    }

    /// Sorts the uploaded splats in front of the eye, farthest first, for a view from
    /// `origin` along `forward` (orthographic: every splat), and uploads the order.
    fn sort(&mut self, gpu: &Gpu, origin: [f32; 3], forward: [f32; 3], ortho: bool) {
        let key = [
            origin[0],
            origin[1],
            origin[2],
            forward[0],
            forward[1],
            forward[2],
            self.uploaded as f32,
        ];
        self.sort_ms = 0.0;
        // A worker's order lands first, even when the camera has moved on since.
        if let Some((for_key, rx)) = &self.pending {
            match rx.try_recv() {
                Ok((order, ms)) => {
                    let for_key = *for_key;
                    self.pending = None;
                    self.take_order(gpu, &order, for_key, ms);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.pending = None,
            }
        }
        if self.sorted_for == Some(key) {
            return;
        }
        // Large sets with an order to draw meanwhile: one sort at a time on a worker.
        if self.uploaded >= ASYNC_SORT_MIN && self.sorted > 0 {
            if self.pending.is_none() {
                let (tx, rx) = std::sync::mpsc::channel();
                let (splats, stride, n) = (self.splats.clone(), self.stride, self.uploaded);
                rayon::spawn(move || {
                    let t = std::time::Instant::now();
                    let order =
                        sort_back_to_front(&splats.positions, stride, n, origin, forward, ortho);
                    // The receiver is gone when the splats were dropped meanwhile.
                    let _ = tx.send((order, t.elapsed().as_secs_f32() * 1e3));
                });
                self.pending = Some((key, rx));
            }
            return;
        }
        let t = std::time::Instant::now();
        let order = sort_back_to_front(
            &self.splats.positions,
            self.stride,
            self.uploaded,
            origin,
            forward,
            ortho,
        );
        self.take_order(gpu, &order, key, t.elapsed().as_secs_f32() * 1e3);
    }

    /// Uploads a sorted order for the view `key`.
    fn take_order(&mut self, gpu: &Gpu, order: &[u32], key: [f32; 7], ms: f32) {
        if !order.is_empty() {
            gpu.queue
                .write_buffer(&self.order, 0, bytemuck::cast_slice(order));
        }
        self.sorted = order.len() as u32;
        self.sorted_for = Some(key);
        self.sort_ms = ms;
    }
}

/// The indices (on the GPU: splat `i * stride` of the file) of the first `n` splats in front
/// of the eye, farthest first. Depths become order-preserving 32-bit keys sorted by two
/// 16-bit counting passes.
pub fn sort_back_to_front(
    positions: &[[f32; 3]],
    stride: usize,
    n: usize,
    origin: [f32; 3],
    forward: [f32; 3],
    ortho: bool,
) -> Vec<u32> {
    use rayon::prelude::*;
    // Key: larger depth → smaller key (farthest first).
    let keyed: Vec<(u32, u32)> = (0..n)
        .into_par_iter()
        .filter_map(|g| {
            let p = positions[g * stride];
            let d = (p[0] - origin[0]) * forward[0]
                + (p[1] - origin[1]) * forward[1]
                + (p[2] - origin[2]) * forward[2];
            if !ortho && d <= 1e-3 || !d.is_finite() {
                return None;
            }
            let b = d.to_bits();
            // Monotonic in d for every float, then reversed.
            let mono = if b & 0x8000_0000 != 0 {
                !b
            } else {
                b | 0x8000_0000
            };
            Some((!mono, g as u32))
        })
        .collect();
    let mut a = keyed;
    let mut tmp = vec![(0u32, 0u32); a.len()];
    for shift in [0u32, 16] {
        let mut count = vec![0usize; 1 << 16];
        for &(k, _) in &a {
            count[((k >> shift) & 0xffff) as usize] += 1;
        }
        let mut sum = 0;
        for c in count.iter_mut() {
            let v = *c;
            *c = sum;
            sum += v;
        }
        for &e in &a {
            let slot = &mut count[((e.0 >> shift) & 0xffff) as usize];
            tmp[*slot] = e;
            *slot += 1;
        }
        std::mem::swap(&mut a, &mut tmp);
    }
    a.into_iter().map(|(_, i)| i).collect()
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SplatU {
    right: [f32; 4],
    down: [f32; 4],
    forward: [f32; 4],
    origin: [f32; 4],
    view: [f32; 4],
    to_file: [[f32; 4]; 3],
}

struct Targets {
    size: (u32, u32),
    color: wgpu::TextureView,
    present: wgpu::BindGroup,
}

/// Draws splats; its pipelines and offscreen target.
pub struct SplatRenderer {
    splat_layout: wgpu::BindGroupLayout,
    present_layout: wgpu::BindGroupLayout,
    splats: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    frame_buf: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    splat_buf: wgpu::Buffer,
    present_buf: wgpu::Buffer,
    targets: Option<Targets>,
}

impl SplatRenderer {
    pub fn new(gpu: &Gpu) -> SplatRenderer {
        let d = &gpu.device;
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let vs = wgpu::ShaderStages::VERTEX;
        let fs = wgpu::ShaderStages::FRAGMENT;
        let frame_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("splats frame"),
            entries: &[layout_entry(0, vf, uniform())],
        });
        let splat_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("splats"),
            entries: &[
                layout_entry(0, vs, uniform()),
                layout_entry(1, vs, storage()),
                layout_entry(2, vs, storage()),
                layout_entry(3, vs, storage()),
            ],
        });
        let present_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("splats present"),
            entries: &[
                layout_entry(0, fs, uniform()),
                layout_entry(
                    1,
                    fs,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
            ],
        });
        let module = shader(d, "splats", &splats_source());
        let pipe = |label, layouts: &[Option<&wgpu::BindGroupLayout>], vs: &str, fs: &str| {
            let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: layouts,
                immediate_size: 0,
            });
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR,
                        blend: Some(PREMULTIPLIED),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: gpu.cache(),
            })
        };
        let splats = pipe(
            "splats",
            &[Some(&frame_layout), Some(&splat_layout)],
            "vs_splat",
            "fs_splat",
        );
        let grid = pipe(
            "splats grid",
            &[Some(&frame_layout)],
            "vs_grid",
            "fs_grid_splats",
        );
        let present_module = shader(d, "splats present", &splat_present_source());
        let present = fullscreen_pipeline(
            gpu,
            "splats present",
            &present_module,
            "fs_splat_present",
            &[Some(&present_layout)],
            HDR,
            Some(PREMULTIPLIED),
        );
        let buffer = |label, size: usize| {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let frame_buf = buffer("splats frame", size_of::<crate::FrameU>());
        let splat_buf = buffer("splats params", size_of::<SplatU>());
        let present_buf = buffer("splats present", 16);
        let frame_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("splats frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });
        SplatRenderer {
            splat_layout,
            present_layout,
            splats,
            grid,
            present,
            frame_buf,
            frame_group,
            splat_buf,
            present_buf,
            targets: None,
        }
    }

    fn targets(&mut self, gpu: &Gpu, size: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let d = &gpu.device;
        let color = d
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("splats colour"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let present = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("splats present"),
            layout: &self.present_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.present_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&color),
                },
            ],
        });
        self.targets = Some(Targets {
            size,
            color,
            present,
        });
    }

    /// Draws `splats` as `view` says into `target` (the HDR scene, `size` px) inside the
    /// view's rectangle, over what is there; sorts them first when the camera moved.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        splats: &mut SplatsGpu,
        view: &SplatView,
    ) {
        let x0 = view.rect[0].min(size.0);
        let y0 = view.rect[1].min(size.1);
        let x1 = view.rect[0].saturating_add(view.rect[2]).min(size.0);
        let y1 = view.rect[1].saturating_add(view.rect[3]).min(size.1);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (w, h) = (view.rect[2].max(1), view.rect[3].max(1));
        let b = splats.bounds;
        let (frame, p) = frame_for(&b, &view.camera, view.grid, w, h);
        let cam = &view.camera;
        let f32v = |v: [f64; 3]| v.map(|c| c as f32);
        let (origin, focal, ortho) = match p.eye {
            Some(e) => (
                f32v(e),
                (f64::from(h) / 2.0 / (cam.fov / 2.0).tan()) as f32,
                false,
            ),
            None => (f32v(p.target), (1.0 / p.units_per_px) as f32, true),
        };
        let forward = f32v(p.forward);
        splats.sort(gpu, origin, forward, ortho);
        let v4 = |a: [f32; 3], w: f32| [a[0], a[1], a[2], w];
        let tf = splats.splats.to_file;
        let su = SplatU {
            right: v4(f32v(cam.right), if ortho { 1.0 } else { 0.0 }),
            down: v4(f32v(cam.down), 0.0),
            forward: v4(forward, 0.0),
            origin: v4(origin, focal),
            view: [
                w as f32,
                h as f32,
                splats.k as f32,
                if splats.splats.antialiased { 1.0 } else { 0.0 },
            ],
            to_file: tf.map(|r| v4(r, 0.0)),
        };
        gpu.queue
            .write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));
        gpu.queue
            .write_buffer(&self.splat_buf, 0, bytemuck::bytes_of(&su));
        let rect = [view.rect[0] as f32, view.rect[1] as f32, w as f32, h as f32];
        gpu.queue
            .write_buffer(&self.present_buf, 0, bytemuck::cast_slice(&rect));
        if splats.group.is_none() {
            splats.group = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("splats"),
                layout: &self.splat_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.splat_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: splats.core.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: splats.sh.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: splats.order.as_entire_binding(),
                    },
                ],
            }));
        }
        self.targets(gpu, (w, h));
        let t = self.targets.as_ref().expect("made above");
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("splats"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.frame_group, &[]);
            if view.grid {
                pass.set_pipeline(&self.grid);
                pass.draw(0..6, 0..1);
            }
            if splats.sorted > 0 {
                pass.set_pipeline(&self.splats);
                pass.set_bind_group(1, splats.group.as_ref().expect("made above"), &[]);
                pass.draw(0..6, 0..splats.sorted);
            }
        }
        splats.drawn = splats.sorted;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("splats present"),
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
        pass.set_pipeline(&self.present);
        pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
        pass.set_bind_group(0, &t.present, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// The splat shader.
pub fn splats_source() -> String {
    format!(
        "{}\n{}\n{}",
        crate::FRAME_WGSL,
        crate::GRID_WGSL,
        include_str!("shaders/splats.wgsl")
    )
}

/// The shader that composites the splat view.
pub fn splat_present_source() -> String {
    format!(
        "{FULLSCREEN_WGSL}\n{}",
        include_str!("shaders/splat_present.wgsl")
    )
}
