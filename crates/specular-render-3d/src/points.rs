//! Point clouds on the GPU. A [`PointsGpu`] follows a [`Loading`]: the batches decoded so far
//! are uploaded as they arrive (drawn in load order while loading), then the finished cloud,
//! sorted into octree leaves, replaces them; at most [`UPLOAD_PER_FRAME`] bytes a frame, so
//! the UI thread never stalls on a big cloud. Points live in pages of at most 64 MB (5 words
//! a point: position relative to the cloud's origin, colour and class, intensity and
//! returns), pulled by instance from a storage buffer.
//!
//! [`PointRenderer::draw`] draws a [`PointView`]: round splats sized in px or model units,
//! coloured by RGB, intensity, elevation (batlow) or ASPRS class, under the mesh view's
//! camera and over its ground grid, with optional eye-dome lighting. Each frame draws at
//! most the view's point budget, picked per leaf by [`specular_points::select`] for the
//! leaves' size on screen.

use std::sync::Arc;

use specular_gfx::{Gpu, HDR, fullscreen_pipeline, shader, wgpu};
use specular_points::{Bounds, CLASS_COLOURS, Channels, Leaf, Loading, PointCloud};

use crate::{
    Camera, DEPTH, PREMULTIPLIED, PipeSpec, edl_source, frame_for, layout_entry, points_source,
    storage, uniform,
};

/// Bytes of one point on the GPU.
const RECORD: usize = 20;

/// Points in a page (64 MB).
pub const PAGE_POINTS: usize = (64 << 20) / RECORD;

/// Bytes uploaded a frame at most.
pub const UPLOAD_PER_FRAME: usize = 16 << 20;

/// The format of the eye-dome lighting target (log depth).
const EDL_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// What colours the points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointColour {
    #[default]
    Rgb,
    Intensity,
    Elevation,
    Classification,
}

impl PointColour {
    pub const ALL: [PointColour; 4] = [
        PointColour::Rgb,
        PointColour::Intensity,
        PointColour::Elevation,
        PointColour::Classification,
    ];

    pub fn id(self) -> &'static str {
        match self {
            PointColour::Rgb => "rgb",
            PointColour::Intensity => "intensity",
            PointColour::Elevation => "elevation",
            PointColour::Classification => "classification",
        }
    }

    /// Whether a cloud with `ch` can be coloured so (elevation always).
    pub fn available(self, ch: &Channels) -> bool {
        match self {
            PointColour::Rgb => ch.rgb,
            PointColour::Intensity => ch.intensity,
            PointColour::Elevation => true,
            PointColour::Classification => ch.classification,
        }
    }

    /// The colouring a cloud opens with: its own colours, else elevation.
    pub fn default_for(ch: &Channels) -> PointColour {
        if ch.rgb {
            PointColour::Rgb
        } else {
            PointColour::Elevation
        }
    }

    /// The next colouring a cloud with `ch` can show.
    pub fn next_for(self, ch: &Channels) -> PointColour {
        let i = PointColour::ALL
            .iter()
            .position(|c| *c == self)
            .unwrap_or(0);
        (1..=4)
            .map(|k| PointColour::ALL[(i + k) % 4])
            .find(|c| c.available(ch))
            .unwrap_or(PointColour::Elevation)
    }

    fn code(self) -> f32 {
        match self {
            PointColour::Rgb => 0.0,
            PointColour::Intensity => 1.0,
            PointColour::Elevation => 2.0,
            PointColour::Classification => 3.0,
        }
    }
}

/// One view of a point cloud.
#[derive(Clone, Copy, Debug)]
pub struct PointView {
    /// Target rectangle: x, y, width, height (physical px).
    pub rect: [u32; 4],
    pub camera: Camera,
    pub grid: bool,
    /// Physical px per logical px.
    pub scale: f32,
    pub colour: PointColour,
    /// The splats' diameter: logical px, or model units when `world`.
    pub size: f32,
    pub world: bool,
    /// Eye-dome lighting.
    pub edl: bool,
    /// Points drawn a frame at most.
    pub budget: u64,
}

struct Page {
    buf: wgpu::Buffer,
    group: wgpu::BindGroup,
    cap: usize,
}

/// Points written into pages, in order.
#[derive(Default)]
struct Pages {
    pages: Vec<Page>,
    len: usize,
}

impl Pages {
    /// Appends points; `hint` is how many more are coming in all (the size of a new page).
    fn push(
        &mut self,
        gpu: &Gpu,
        layout: &wgpu::BindGroupLayout,
        positions: &[[f32; 3]],
        attrs: &[[u32; 2]],
        hint: usize,
    ) {
        let mut done = 0;
        while done < positions.len() {
            let mut used: usize = self.pages.iter().map(|p| p.cap).sum();
            if used == self.len {
                let cap = hint
                    .saturating_sub(done)
                    .max(positions.len() - done)
                    .clamp(1, PAGE_POINTS);
                let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("points page"),
                    size: (cap * RECORD) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("points page"),
                    layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    }],
                });
                self.pages.push(Page { buf, group, cap });
                used += cap;
            }
            let page = self.pages.last().expect("made above");
            let at = self.len - (used - page.cap);
            let n = (page.cap - at).min(positions.len() - done);
            let mut bytes = Vec::with_capacity(n * RECORD);
            for (p, a) in positions[done..done + n].iter().zip(&attrs[done..done + n]) {
                for w in [p[0].to_bits(), p[1].to_bits(), p[2].to_bits(), a[0], a[1]] {
                    bytes.extend_from_slice(&w.to_le_bytes());
                }
            }
            gpu.queue
                .write_buffer(&page.buf, (at * RECORD) as u64, &bytes);
            self.len += n;
            done += n;
        }
    }

    fn bytes(&self) -> u64 {
        self.pages.iter().map(|p| (p.cap * RECORD) as u64).sum()
    }
}

/// A cloud on the GPU, following its loading.
pub struct PointsGpu {
    /// The box of the points (relative to the cloud's origin): the header's while loading.
    pub bounds: Bounds,
    /// What is drawn: the loaded batches, then the finished cloud.
    live: Pages,
    /// The finished cloud's leaves, once `live` holds it.
    leaves: Vec<Leaf>,
    /// The finished cloud being uploaded.
    pending: Option<(Arc<PointCloud>, Pages)>,
    /// Batches of the loading uploaded.
    batches: usize,
    intensity: [u16; 2],
    done: bool,
    /// Points drawn by the last frame.
    pub drawn: u64,
}

impl PointsGpu {
    /// Nothing on the GPU yet; the box from the header.
    pub fn new(loading: &Loading) -> PointsGpu {
        PointsGpu {
            bounds: loading.bounds,
            live: Pages::default(),
            leaves: Vec::new(),
            pending: None,
            batches: 0,
            intensity: loading.intensity_range(),
            done: false,
            drawn: 0,
        }
    }

    /// The finished cloud is on the GPU (or loading failed, leaving what had arrived).
    pub fn complete(&self) -> bool {
        self.done
    }

    /// Points on the GPU.
    pub fn len(&self) -> usize {
        self.live.len
    }

    pub fn is_empty(&self) -> bool {
        self.live.len == 0
    }

    /// Bytes on the GPU.
    pub fn bytes(&self) -> u64 {
        self.live.bytes() + self.pending.as_ref().map_or(0, |(_, p)| p.bytes())
    }

    /// Uploads what `loading` has that the GPU has not, at most [`UPLOAD_PER_FRAME`] bytes;
    /// true when what is drawn changed.
    pub fn feed(&mut self, r: &PointRenderer, gpu: &Gpu, loading: &Loading) -> bool {
        if self.done {
            return false;
        }
        let mut left = UPLOAD_PER_FRAME;
        self.intensity = loading.intensity_range();
        match loading.result() {
            Some(Ok(cloud)) => {
                let (c, pages) = self
                    .pending
                    .get_or_insert_with(|| (cloud, Pages::default()));
                while pages.len < c.len() && left > 0 {
                    let n = (left / RECORD).min(c.len() - pages.len).max(1);
                    let at = pages.len;
                    let rest = c.len() - at;
                    pages.push(
                        gpu,
                        &r.page_layout,
                        &c.positions[at..at + n],
                        &c.attrs[at..at + n],
                        rest,
                    );
                    left = left.saturating_sub(n * RECORD);
                }
                if pages.len < c.len() {
                    return false;
                }
                let (c, pages) = self.pending.take().expect("set above");
                self.live = pages;
                self.leaves = c.leaves.clone();
                self.bounds = c.bounds;
                self.intensity = c.info.intensity_range;
                self.done = true;
                true
            }
            Some(Err(_)) => {
                self.done = true;
                false
            }
            None => {
                let mut changed = false;
                let expected = usize::try_from(loading.expected).unwrap_or(usize::MAX);
                for b in loading.batches(self.batches) {
                    if left == 0 {
                        break;
                    }
                    let hint = expected.saturating_sub(self.live.len);
                    self.live
                        .push(gpu, &r.page_layout, &b.positions, &b.attrs, hint);
                    left = left.saturating_sub(b.positions.len() * RECORD);
                    if self.bounds.is_empty() {
                        for &p in &b.positions {
                            self.bounds.add(p);
                        }
                    }
                    self.batches += 1;
                    changed = true;
                }
                changed
            }
        }
    }

    /// The `(first, count)` runs a view draws.
    fn runs(&self, view: &specular_points::View, budget: u64) -> Vec<(u32, u32)> {
        if self.leaves.is_empty() || !self.done {
            let n = (self.live.len as u64).min(budget);
            return if n > 0 {
                vec![(0, n as u32)]
            } else {
                Vec::new()
            };
        }
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for (first, count) in specular_points::select(&self.leaves, view, budget) {
            match runs.last_mut() {
                Some(last) if last.0 + last.1 == first => last.1 += count,
                _ => runs.push((first, count)),
            }
        }
        runs
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PointsU {
    mode: [f32; 4],
    ranges: [f32; 4],
    view: [f32; 4],
    depth: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EdlU {
    rect: [f32; 4],
    params: [f32; 4],
}

struct Targets {
    size: (u32, u32),
    color: wgpu::TextureView,
    edl: wgpu::TextureView,
    depth: wgpu::TextureView,
    present: wgpu::BindGroup,
}

/// Draws point clouds; its pipelines, the colour tables and offscreen targets.
pub struct PointRenderer {
    pub(crate) page_layout: wgpu::BindGroupLayout,
    edl_layout: wgpu::BindGroupLayout,
    points: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    frame_buf: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    /// Section planes the next draws cut with, and how many.
    clip: ([[f32; 4]; 12], u32),
    params_buf: wgpu::Buffer,
    params_group: wgpu::BindGroup,
    edl_buf: wgpu::Buffer,
    targets: Option<Targets>,
    /// The box the next draws are framed on, in place of the cloud's own.
    frame: Option<Bounds>,
    /// Copies a mesh's depth under the points (made on first use, per sample count).
    fill: Option<DepthFill>,
}

/// A pass writing a drawn mesh's depth into the points' depth buffer, so points behind the
/// mesh fail the depth test.
struct DepthFill {
    samples: u32,
    layout: wgpu::BindGroupLayout,
    pipe: wgpu::RenderPipeline,
}

/// The depth fill's shader: the nearest of the pixel's samples, set back a little so points
/// lying on a surface stay in front of it.
pub(crate) fn depth_fill_source(samples: u32) -> String {
    let (ty, read) = if samples > 1 {
        (
            "texture_depth_multisampled_2d",
            format!(
                "for (var s = 0; s < {samples}; s++) {{ z = max(z, textureLoad(scene, p, s)); }}"
            ),
        )
    } else {
        (
            "texture_depth_2d",
            "z = textureLoad(scene, p, 0);".to_string(),
        )
    };
    format!(
        r#"@group(0) @binding(0) var scene: {ty};

@vertex
fn vs_fill(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
}}

@fragment
fn fs_fill(@builtin(position) pos: vec4<f32>) -> @builtin(frag_depth) f32 {{
    let p = vec2<i32>(floor(pos.xy));
    var z = 0.0;
    {read}
    return z * 0.9995;
}}
"#
    )
}

impl DepthFill {
    fn new(gpu: &Gpu, samples: u32) -> DepthFill {
        let d = &gpu.device;
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("points depth fill"),
            entries: &[layout_entry(
                0,
                wgpu::ShaderStages::FRAGMENT,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: samples > 1,
                },
            )],
        });
        let module = shader(d, "points depth fill", &depth_fill_source(samples));
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("points depth fill"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("points depth fill"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_fill"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_fill"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: gpu.cache(),
        });
        DepthFill {
            samples,
            layout,
            pipe,
        }
    }
}

/// The colour tables: batlow over elevation (row 0), the ASPRS class colours (row 1).
fn lut() -> Vec<u8> {
    let mut px = Vec::with_capacity(256 * 2 * 4);
    for c in specular_inspect::ColourMap::Batlow.table() {
        px.extend(c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        px.push(255);
    }
    for c in CLASS_COLOURS {
        px.extend(c);
        px.push(255);
    }
    px
}

impl PointRenderer {
    pub fn new(gpu: &Gpu) -> PointRenderer {
        let d = &gpu.device;
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let vs = wgpu::ShaderStages::VERTEX;
        let fs = wgpu::ShaderStages::FRAGMENT;
        let frame_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("points frame"),
            entries: &[layout_entry(0, vf, uniform())],
        });
        let unfiltered = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let params_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("points params"),
            entries: &[
                layout_entry(0, vf, uniform()),
                layout_entry(1, vs, unfiltered),
            ],
        });
        let page_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("points page"),
            entries: &[layout_entry(0, vs, storage())],
        });
        let edl_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("points edl"),
            entries: &[
                layout_entry(0, fs, uniform()),
                layout_entry(1, fs, unfiltered),
                layout_entry(2, fs, unfiltered),
            ],
        });
        let module = shader(d, "points", &points_source());
        let points_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("points"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&params_layout),
                Some(&page_layout),
            ],
            immediate_size: 0,
        });
        let grid_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("points grid"),
            bind_group_layouts: &[Some(&frame_layout)],
            immediate_size: 0,
        });
        let pipe = |layout: &wgpu::PipelineLayout, s: PipeSpec, depth_mask: wgpu::ColorWrites| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(s.label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(s.vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(s.depth_write),
                    depth_compare: Some(s.compare),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(s.fs),
                    compilation_options: Default::default(),
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: HDR,
                            blend: s.blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                        Some(wgpu::ColorTargetState {
                            format: EDL_DEPTH,
                            blend: None,
                            write_mask: depth_mask,
                        }),
                    ],
                }),
                multiview_mask: None,
                cache: gpu.cache(),
            })
        };
        use wgpu::CompareFunction as C;
        let points = pipe(
            &points_layout,
            PipeSpec {
                label: "points",
                vs: "vs_points",
                fs: "fs_points",
                blend: None,
                depth_write: true,
                compare: C::Greater,
            },
            wgpu::ColorWrites::ALL,
        );
        let grid = pipe(
            &grid_layout,
            PipeSpec {
                label: "points grid",
                vs: "vs_grid",
                fs: "fs_grid_points",
                blend: Some(PREMULTIPLIED),
                depth_write: false,
                compare: C::GreaterEqual,
            },
            wgpu::ColorWrites::empty(),
        );
        let edl_module = shader(d, "points edl", &edl_source());
        let present = fullscreen_pipeline(
            gpu,
            "points edl",
            &edl_module,
            "fs_edl",
            &[Some(&edl_layout)],
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
        let frame_buf = buffer("points frame", size_of::<crate::FrameU>());
        let params_buf = buffer("points params", size_of::<PointsU>());
        let edl_buf = buffer("points edl", size_of::<EdlU>());
        let frame_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("points frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });
        let size = wgpu::Extent3d {
            width: 256,
            height: 2,
            depth_or_array_layers: 1,
        };
        let lut_tex = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("points colours"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &lut_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &lut(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: None,
            },
            size,
        );
        let lut_view = lut_tex.create_view(&Default::default());
        let params_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("points params"),
            layout: &params_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view),
                },
            ],
        });
        PointRenderer {
            page_layout,
            edl_layout,
            points,
            grid,
            present,
            frame_buf,
            frame_group,
            clip: ([[0.0; 4]; 12], 0),
            params_buf,
            params_group,
            edl_buf,
            targets: None,
            frame: None,
            fill: None,
        }
    }

    fn targets(&mut self, gpu: &Gpu, size: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let d = &gpu.device;
        let tex = |label, format, usage| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&Default::default())
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let read = attach | wgpu::TextureUsages::TEXTURE_BINDING;
        let color = tex("points colour", HDR, read);
        let edl = tex("points log depth", EDL_DEPTH, read);
        let depth = tex("points depth", DEPTH, attach);
        let present = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("points edl"),
            layout: &self.edl_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.edl_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&color),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&edl),
                },
            ],
        });
        self.targets = Some(Targets {
            size,
            color,
            edl,
            depth,
            present,
        });
    }

    /// Draws `cloud` as `view` says into `target` (the HDR scene, `size` px) inside the
    /// view's rectangle, over what is there.
    /// Cuts the next draws with `s` (its planes and box).
    pub fn set_sections(&mut self, s: &crate::section::Sections) {
        self.clip = s.uniform();
    }

    /// Frames the next draws on `b` instead of the cloud's own box (as
    /// [`crate::MeshRenderer::set_frame`]; the same box lines the points up with a mesh).
    pub fn set_frame(&mut self, b: Option<Bounds>) {
        self.frame = b;
    }

    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        cloud: &mut PointsGpu,
        view: &PointView,
    ) {
        self.draw_over(gpu, enc, target, size, cloud, view, None);
    }

    /// [`PointRenderer::draw`] over a mesh just drawn in the same view and frame: points
    /// behind the mesh (`under`, [`crate::MeshRenderer::depth`]) are hidden.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_over(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        cloud: &mut PointsGpu,
        view: &PointView,
        under: Option<crate::SceneDepth<'_>>,
    ) {
        let x0 = view.rect[0].min(size.0);
        let y0 = view.rect[1].min(size.1);
        let x1 = view.rect[0].saturating_add(view.rect[2]).min(size.0);
        let y1 = view.rect[1].saturating_add(view.rect[3]).min(size.1);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (w, h) = (view.rect[2].max(1), view.rect[3].max(1));
        let b = cloud.bounds;
        let (mut frame, p) = frame_for(&self.frame.unwrap_or(b), &view.camera, view.grid, w, h);
        frame.clip = self.clip.0;
        frame.clip_n[0] = self.clip.1 as f32;
        let len3 = |r: &[f64; 4]| (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        let ortho = p.eye.is_none();
        let reach = 0.5 / len3(&p.rows[2]).max(1e-30);
        let z = if b.is_empty() {
            [0.0, 1.0]
        } else {
            [b.min[2], b.max[2]]
        };
        let params = PointsU {
            mode: [
                view.colour.code(),
                if view.world { 1.0 } else { 0.0 },
                view.size.max(0.1),
                view.scale.max(0.1),
            ],
            ranges: [
                z[0],
                z[1],
                f32::from(cloud.intensity[0]),
                f32::from(cloud.intensity[1]),
            ],
            view: [
                w as f32,
                h as f32,
                (len3(&p.rows[1]) * f64::from(h) / 2.0) as f32,
                0.0,
            ],
            depth: [
                if ortho { 1.0 } else { 0.0 },
                reach as f32,
                (2.0 * reach) as f32,
                0.0,
            ],
        };
        gpu.queue
            .write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));
        gpu.queue
            .write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&params));
        let edl = EdlU {
            rect: [view.rect[0] as f32, view.rect[1] as f32, w as f32, h as f32],
            params: [
                if view.edl { 1.0 } else { 0.0 },
                view.scale.max(1.0).round(),
                0.0,
                0.0,
            ],
        };
        gpu.queue
            .write_buffer(&self.edl_buf, 0, bytemuck::bytes_of(&edl));
        // Splats (at random places) this far apart on screen look filled: half their
        // diameter, which for model-unit splats is taken at the middle of the cloud.
        let diameter = if view.world {
            let c = b.centre().map(f64::from);
            let r3 = &p.rows[3];
            let depth = if ortho {
                1.0
            } else {
                (r3[0] * c[0] + r3[1] * c[1] + r3[2] * c[2] + r3[3]).max(1e-9)
            };
            (f64::from(view.size) * f64::from(params.view[2]) / depth) as f32
        } else {
            view.size * view.scale
        };
        let spacing = (0.5 * diameter).clamp(0.5, 32.0);
        let lod = specular_points::View {
            rows: p.rows.map(|r| r.map(|v| v as f32)),
            width: w as f32,
            height: h as f32,
            spacing_px: spacing,
        };
        let runs = cloud.runs(&lod, view.budget);
        cloud.drawn = runs.iter().map(|r| u64::from(r.1)).sum();
        self.targets(gpu, (w, h));
        let under = under.filter(|u| u.size == (w, h));
        if let Some(u) = under {
            if self.fill.as_ref().is_none_or(|f| f.samples != u.samples) {
                self.fill = Some(DepthFill::new(gpu, u.samples));
            }
            let (Some(f), Some(t)) = (self.fill.as_ref(), self.targets.as_ref()) else {
                return;
            };
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("points depth fill"),
                layout: &f.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(u.view),
                }],
            });
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("points depth fill"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&f.pipe);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        let t = self.targets.as_ref().expect("made above");
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("points"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &t.color,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &t.edl,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if under.is_some() {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(0.0)
                        },
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.frame_group, &[]);
            pass.set_pipeline(&self.points);
            pass.set_bind_group(1, &self.params_group, &[]);
            let mut page_first = 0usize;
            for page in &cloud.live.pages {
                let page_end = page_first + page.cap;
                pass.set_bind_group(2, &page.group, &[]);
                for &(first, count) in &runs {
                    let (a, e) = (first as usize, first as usize + count as usize);
                    let (a, e) = (a.max(page_first), e.min(page_end).min(cloud.live.len));
                    if a < e {
                        pass.draw(0..6, (a - page_first) as u32..(e - page_first) as u32);
                    }
                }
                page_first = page_end;
            }
            if view.grid {
                pass.set_pipeline(&self.grid);
                pass.draw(0..6, 0..1);
            }
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("points present"),
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
