//! Frame composition. The image renderer draws into `scene` (linear, half-float) only when
//! the view changes. Each frame the scene is copied into `comp`, the UI is drawn on top, and
//! the final pass encodes to the swapchain with blue-noise dither. Glass samples a
//! dual-Kawase blurred copy of the scene.

use crate::gpu::{Gpu, shader};

mod display;

pub const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BLUE_NOISE: &[u8] = include_bytes!("../assets/blue_noise_64.r8");
/// WGSL for a full-screen triangle (`vs_full`), to prepend to fragment shaders.
pub const FULLSCREEN_WGSL: &str = include_str!("shaders/fullscreen.wgsl");
const FULLSCREEN: &str = FULLSCREEN_WGSL;

pub struct Target {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl Target {
    pub fn new(
        device: &wgpu::Device,
        label: &str,
        w: u32,
        h: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Target {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Target {
            texture,
            view,
            width: w.max(1),
            height: h.max(1),
        }
    }
}

struct BlurPass {
    bind: wgpu::BindGroup,
    dst: usize,
    up: bool,
}

pub struct Compositor {
    pub scene: Target,
    pub comp: Target,
    /// d1..d4 (½ … 1⁄16), then u3 (⅛) and u2 (¼): the backdrop.
    blur: Vec<Target>,
    passes: Vec<BlurPass>,
    down_pipe: wgpu::RenderPipeline,
    up_pipe: wgpu::RenderPipeline,
    blur_bgl: wgpu::BindGroupLayout,
    final_pipe: wgpu::RenderPipeline,
    final_mod: wgpu::ShaderModule,
    /// Swapchain format `final_pipe` writes.
    out_format: wgpu::TextureFormat,
    final_bgl: wgpu::BindGroupLayout,
    final_bind: wgpu::BindGroup,
    /// The final pass reading the scene itself, for frames with nothing drawn over it.
    scene_bind: wgpu::BindGroup,
    final_buf: wgpu::Buffer,
    noise_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    params: Vec<wgpu::Buffer>,
    /// Display transform table of the final pass (see `display`).
    display_lut: wgpu::TextureView,
    /// Colour-vision-deficiency simulation of the final pass (see `display`).
    cvd: Option<[[f32; 3]; 3]>,
}

impl std::fmt::Debug for Compositor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compositor")
            .field("w", &self.scene.width)
            .field("h", &self.scene.height)
            .finish()
    }
}

fn tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// A full-screen-triangle pipeline.
pub fn fullscreen_pipeline(
    gpu: &Gpu,
    label: &str,
    module: &wgpu::ShaderModule,
    fs: &str,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: layouts,
            immediate_size: 0,
        });
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_full"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: gpu.cache(),
        })
}

impl Compositor {
    pub fn new(
        gpu: &Gpu,
        surface_format: wgpu::TextureFormat,
        bits: u32,
        w: u32,
        h: u32,
    ) -> Compositor {
        let d = &gpu.device;
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let blur_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur"),
            entries: &[
                tex_entry(0),
                sampler_entry(1),
                uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let blur_mod = shader(
            d,
            "blur",
            &format!("{FULLSCREEN}\n{}", include_str!("shaders/blur.wgsl")),
        );
        let down_pipe = fullscreen_pipeline(
            gpu,
            "blur down",
            &blur_mod,
            "fs_down",
            &[Some(&blur_bgl)],
            HDR,
            None,
        );
        let up_pipe = fullscreen_pipeline(
            gpu,
            "blur up",
            &blur_mod,
            "fs_up",
            &[Some(&blur_bgl)],
            HDR,
            None,
        );

        let final_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("final"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                sampler_entry(4),
            ],
        });
        let final_mod = shader(
            d,
            "final",
            &format!("{FULLSCREEN}\n{}", include_str!("shaders/final.wgsl")),
        );
        let final_pipe = fullscreen_pipeline(
            gpu,
            "final",
            &final_mod,
            "fs_final",
            &[Some(&final_bgl)],
            surface_format,
            None,
        );

        let noise = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("blue noise"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            noise.as_image_copy(),
            BLUE_NOISE,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(64),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
        let noise_view = noise.create_view(&Default::default());
        let lsb = 1.0 / ((1u32 << bits) - 1) as f32;
        let final_buf = uniform_buffer(d, "final params", &display::params(lsb));
        let params = (0..6)
            .map(|_| uniform_buffer(d, "blur params", &[0.0f32; 4]))
            .collect();

        let scene = Target::new(d, "scene", w, h, HDR, scene_usage());
        let comp = Target::new(d, "comp", w, h, HDR, comp_usage());
        let display_lut = display::identity_lut(gpu);
        let [final_bind, scene_bind] = [&comp, &scene].map(|t| {
            final_bind(
                d,
                &final_bgl,
                t,
                &noise_view,
                &final_buf,
                &display_lut,
                &sampler,
            )
        });
        let mut c = Compositor {
            scene,
            comp,
            blur: Vec::new(),
            passes: Vec::new(),
            down_pipe,
            up_pipe,
            blur_bgl,
            final_pipe,
            final_mod,
            out_format: surface_format,
            final_bgl,
            final_bind,
            scene_bind,
            final_buf,
            noise_view,
            sampler,
            params,
            display_lut,
            cvd: None,
        };
        c.build_blur(gpu);
        c
    }

    pub fn size(&self) -> (u32, u32) {
        (self.scene.width, self.scene.height)
    }

    pub fn resize(&mut self, gpu: &Gpu, w: u32, h: u32) {
        if (w.max(1), h.max(1)) == self.size() {
            return;
        }
        let d = &gpu.device;
        self.scene = Target::new(d, "scene", w, h, HDR, scene_usage());
        self.comp = Target::new(d, "comp", w, h, HDR, comp_usage());
        self.rebind_final(d);
        self.build_blur(gpu);
    }

    /// The final pass's bind groups, after a target or the display LUT changed.
    fn rebind_final(&mut self, d: &wgpu::Device) {
        [self.final_bind, self.scene_bind] = [&self.comp, &self.scene].map(|t| {
            final_bind(
                d,
                &self.final_bgl,
                t,
                &self.noise_view,
                &self.final_buf,
                &self.display_lut,
                &self.sampler,
            )
        });
    }

    pub fn set_bits(&self, gpu: &Gpu, bits: u32) {
        let lsb = 1.0 / ((1u32 << bits) - 1) as f32;
        gpu.queue
            .write_buffer(&self.final_buf, 0, bytemuck::cast_slice(&[lsb]));
    }

    /// Follows the swapchain: `format` is its (new) format; `hdr_peak` is `Some(headroom)`
    /// for an HDR swapchain and `None` for SDR (sRGB encoding with dither, see
    /// [`Compositor::set_bits`]). HDR is scRGB (half float: linear output clamped to the
    /// headroom, no encoding or dither), or BT.2100 PQ for a 10-bit swapchain (the
    /// fallback where the platform offers no scRGB, see
    /// [`crate::surface::WindowSurface::set_hdr`]): Rec. 2020, SDR white at 203 cd/m².
    pub fn set_output(&mut self, gpu: &Gpu, format: wgpu::TextureFormat, hdr_peak: Option<f32>) {
        if format != self.out_format {
            self.final_pipe = fullscreen_pipeline(
                gpu,
                "final",
                &self.final_mod,
                "fs_final",
                &[Some(&self.final_bgl)],
                format,
                None,
            );
            self.out_format = format;
        }
        let mode = if format == wgpu::TextureFormat::Rgb10a2Unorm {
            2.0f32
        } else {
            1.0
        };
        let (linear, peak) = match hdr_peak {
            Some(p) if p.is_finite() => (mode, p.max(1.0)),
            Some(_) => (mode, 1.0),
            None => (0.0, 1.0),
        };
        gpu.queue
            .write_buffer(&self.final_buf, 4, bytemuck::cast_slice(&[linear, peak]));
    }

    fn build_blur(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        let (w, h) = self.size();
        let dims = [
            (w / 2, h / 2),
            (w / 4, h / 4),
            (w / 8, h / 8),
            (w / 16, h / 16),
            (w / 8, h / 8),
            (w / 4, h / 4),
        ];
        self.blur = dims
            .iter()
            .map(|&(bw, bh)| {
                Target::new(
                    d,
                    "blur",
                    bw.max(1),
                    bh.max(1),
                    HDR,
                    wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        // Read back by the tests.
                        | if cfg!(test) {
                            wgpu::TextureUsages::COPY_SRC
                        } else {
                            wgpu::TextureUsages::empty()
                        },
                )
            })
            .collect();
        // (source, destination, up?)
        let plan: [(Option<usize>, usize, bool); 6] = [
            (None, 0, false),
            (Some(0), 1, false),
            (Some(1), 2, false),
            (Some(2), 3, false),
            (Some(3), 4, true),
            (Some(4), 5, true),
        ];
        self.passes = plan
            .iter()
            .enumerate()
            .map(|(i, &(src, dst, up))| {
                let (sv, sw, sh) = match src {
                    None => (&self.scene.view, self.scene.width, self.scene.height),
                    Some(s) => (&self.blur[s].view, self.blur[s].width, self.blur[s].height),
                };
                let offset = if up { 1.0 } else { 1.0 + i as f32 * 0.5 };
                gpu.queue.write_buffer(
                    &self.params[i],
                    0,
                    bytemuck::cast_slice(&[0.5 / sw as f32, 0.5 / sh as f32, offset, 0.0]),
                );
                let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("blur"),
                    layout: &self.blur_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(sv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.params[i].as_entire_binding(),
                        },
                    ],
                });
                BlurPass { bind, dst, up }
            })
            .collect();
    }

    /// The blurred scene the glass primitives sample.
    pub fn backdrop_view(&self) -> &wgpu::TextureView {
        &self.blur[5].view
    }

    /// Blurs the scene into the backdrop. Only needed when the scene changed and glass is
    /// on screen.
    pub fn blur(&self, enc: &mut wgpu::CommandEncoder) {
        for p in &self.passes {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.blur[p.dst].view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            rp.set_pipeline(if p.up { &self.up_pipe } else { &self.down_pipe });
            rp.set_bind_group(0, &p.bind, &[]);
            rp.draw(0..3, 0..1);
        }
    }

    /// Starts the UI layer: the composite becomes a copy of the scene.
    pub fn begin(&self, enc: &mut wgpu::CommandEncoder) {
        enc.copy_texture_to_texture(
            self.scene.texture.as_image_copy(),
            self.comp.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.scene.width,
                height: self.scene.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Encodes and dithers the composite into the swapchain image.
    pub fn finish(&self, enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        self.finish_from(enc, target, true);
    }

    /// [`Compositor::finish`] from the composite (`composited`: [`Compositor::begin`] ran
    /// and things were drawn over the scene) or straight from the scene, for a frame with
    /// nothing over it: the same pixels without the full-window copy.
    pub fn finish_from(
        &self,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        composited: bool,
    ) {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("final"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        rp.set_pipeline(&self.final_pipe);
        let bind = if composited {
            &self.final_bind
        } else {
            &self.scene_bind
        };
        rp.set_bind_group(0, bind, &[]);
        rp.draw(0..3, 0..1);
    }
}

fn scene_usage() -> wgpu::TextureUsages {
    wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC
}

fn comp_usage() -> wgpu::TextureUsages {
    wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_DST
        | wgpu::TextureUsages::COPY_SRC
}

fn final_bind(
    d: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    comp: &Target,
    noise: &wgpu::TextureView,
    buf: &wgpu::Buffer,
    lut: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("final"),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&comp.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(noise),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(lut),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

pub fn uniform_buffer(d: &wgpu::Device, label: &str, data: &[f32]) -> wgpu::Buffer {
    use crate::gpu::BufferInit;
    d.buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

#[cfg(test)]
mod tests {
    use specular_color::math::f16_to_f32;

    use super::*;

    fn clear(enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, c: wgpu::Color) {
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(c),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }

    /// ST 2084 inverse EOTF of `l` units of SDR white (203 cd/m²).
    fn pq(l: f64) -> f64 {
        let y = (l * 203.0 / 10_000.0).clamp(0.0, 1.0);
        let p = y.powf(0.159_301_757_812_5);
        ((0.835_937_5 + 18.851_562_5 * p) / (1.0 + 18.687_5 * p)).powf(78.843_75)
    }

    /// A 10-bit BT.2100 PQ swapchain (the fallback without scRGB): SDR white is PQ 0.58
    /// (BT.2408), brighter values up to the headroom, above it clamped; greys stay grey in
    /// Rec. 2020. Skips without a GPU.
    #[test]
    fn pq_output_encodes_scrgb_values() {
        let Ok(gpu) = Gpu::new() else {
            eprintln!("no GPU, skipped");
            return;
        };
        let f = wgpu::TextureFormat::Rgb10a2Unorm;
        let (w, h) = (64u32, 4u32);
        let mut comp = Compositor::new(&gpu, f, 10, w, h);
        comp.set_output(&gpu, f, Some(4.0));
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pq target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: f,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        for (v, want) in [(1.0, pq(1.0)), (2.5, pq(2.5)), (9.0, pq(4.0)), (0.0, 0.0)] {
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            clear(
                &mut enc,
                &comp.scene.view,
                wgpu::Color {
                    r: v,
                    g: v,
                    b: v,
                    a: 1.0,
                },
            );
            comp.begin(&mut enc);
            comp.finish(&mut enc, &view);
            let row = 256u32;
            let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pq readback"),
                size: u64::from(row * h),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
                        rows_per_image: Some(h),
                    },
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            gpu.queue.submit([enc.finish()]);
            buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            let data = buf.slice(..).get_mapped_range().expect("mapped");
            let px = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
            let ch = [px & 1023, (px >> 10) & 1023, (px >> 20) & 1023];
            let code = (want * 1023.0).round() as i64;
            for c in ch {
                assert!((i64::from(c) - code).abs() <= 1, "{v}: {ch:?}, want {code}");
            }
        }
        assert!((pq(1.0) - 0.5807).abs() < 1e-3);
    }

    /// The backdrop the glass samples is the blurred scene alone: chrome drawn into the
    /// composite (before or after a blur) never reaches it, so glass can't show a blurred
    /// copy of itself. Skips without a GPU.
    #[test]
    fn backdrop_sees_only_the_scene() {
        let Ok(gpu) = Gpu::new() else {
            eprintln!("no GPU, skipped");
            return;
        };
        let mut comp = Compositor::new(&gpu, wgpu::TextureFormat::Rgba16Float, 16, 96, 64);
        let red = wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let green = wgpu::Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };
        // Two frames, the second after a resize (new targets, rebound passes); each draws the
        // "chrome" (green) over the whole composite between two blurs of a red scene.
        for size in [(96, 64), (160, 100)] {
            comp.resize(&gpu, size.0, size.1);
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            clear(&mut enc, &comp.scene.view, red);
            comp.blur(&mut enc);
            comp.begin(&mut enc);
            clear(&mut enc, &comp.comp.view, green);
            comp.blur(&mut enc);
            let b = &comp.blur[5];
            let row = (b.width * 8).div_ceil(256) * 256;
            let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("backdrop readback"),
                size: (row * b.height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                b.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
                        rows_per_image: Some(b.height),
                    },
                },
                wgpu::Extent3d {
                    width: b.width,
                    height: b.height,
                    depth_or_array_layers: 1,
                },
            );
            gpu.queue.submit([enc.finish()]);
            buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            let data = buf.slice(..).get_mapped_range().expect("mapped");
            for y in 0..b.height as usize {
                for x in 0..b.width as usize {
                    let c = |i: usize| {
                        let o = y * row as usize + x * 8 + 2 * i;
                        f16_to_f32(u16::from_le_bytes([data[o], data[o + 1]]))
                    };
                    let (r, g) = (c(0), c(1));
                    assert!(
                        (r - 1.0).abs() < 1e-3 && g.abs() < 1e-3,
                        "{size:?} backdrop ({x}, {y}) = ({r}, {g})"
                    );
                }
            }
        }
    }

    /// The final pass straight from the scene gives the bytes of copying it into the
    /// composite first (dither included), before and after a resize. Skips without a GPU.
    #[test]
    fn final_pass_from_the_scene_matches_the_composite() {
        let Ok(gpu) = Gpu::new() else {
            eprintln!("no GPU, skipped");
            return;
        };
        let f = wgpu::TextureFormat::Bgra8Unorm;
        let mut comp = Compositor::new(&gpu, f, 8, 64, 48);
        for (w, h) in [(64u32, 48u32), (130, 70)] {
            comp.resize(&gpu, w, h);
            let read = |composited: bool| {
                let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("final test"),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: f,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let mut enc = gpu.device.create_command_encoder(&Default::default());
                clear(
                    &mut enc,
                    &comp.scene.view,
                    wgpu::Color {
                        r: 0.3,
                        g: 0.55,
                        b: 0.9,
                        a: 1.0,
                    },
                );
                if composited {
                    comp.begin(&mut enc);
                }
                comp.finish_from(
                    &mut enc,
                    &target.create_view(&Default::default()),
                    composited,
                );
                let row = (w * 4).div_ceil(256) * 256;
                let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("final readback"),
                    size: u64::from(row * h),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                enc.copy_texture_to_buffer(
                    target.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buf,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row),
                            rows_per_image: Some(h),
                        },
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
                gpu.queue.submit([enc.finish()]);
                buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
                let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
                buf.slice(..).get_mapped_range().expect("mapped").to_vec()
            };
            let (a, b) = (read(true), read(false));
            assert!(a == b, "{w}×{h}: the final pass differs without the copy");
            assert!(a.iter().any(|&v| v != 0));
        }
    }

    /// GPU time of a 4K frame's composite with and without the scene's copy into the
    /// composite: 20 frames per round, paired rounds, best of each. Skips without a GPU.
    #[test]
    fn composite_time_with_and_without_the_scene_copy() {
        let Ok(gpu) = Gpu::new() else {
            eprintln!("no GPU, skipped");
            return;
        };
        let f = wgpu::TextureFormat::Bgra8Unorm;
        let (w, h) = (3840u32, 2160u32);
        let comp = Compositor::new(&gpu, f, 8, w, h);
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("4k target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: f,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let frames = |copy: bool| {
            let t0 = std::time::Instant::now();
            for _ in 0..20 {
                let mut enc = gpu.device.create_command_encoder(&Default::default());
                if copy {
                    comp.begin(&mut enc);
                }
                comp.finish_from(&mut enc, &view, copy);
                gpu.queue.submit([enc.finish()]);
            }
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            t0.elapsed() / 20
        };
        frames(true);
        let (mut with, mut without) = (std::time::Duration::MAX, std::time::Duration::MAX);
        for _ in 0..7 {
            with = with.min(frames(true));
            without = without.min(frames(false));
        }
        eprintln!("4K composite per frame: with the copy {with:.2?}, without {without:.2?}");
    }
}
