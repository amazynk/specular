//! The sound page's backdrop: its cover blurred on the GPU (`blur.wgsl`) into a small
//! texture once a cover, then drawn behind the page under a scrim.

use specular_gfx::{Gpu, HDR, wgpu};

/// The blurred picture's size (texels a side).
const SIZE: u32 = 96;

pub(super) struct Backdrop {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The two passes' uniforms (across, down).
    uniforms: [wgpu::Buffer; 2],
    /// Across, then down: the second is what is drawn.
    targets: Option<[(wgpu::Texture, wgpu::TextureView); 2]>,
    /// The blur of the cover now uploaded is still to be drawn: its bind groups.
    pending: Option<[wgpu::BindGroup; 2]>,
}

fn target(gpu: &Gpu, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sound backdrop"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}

impl Backdrop {
    pub(super) fn new(gpu: &Gpu) -> Backdrop {
        let module = specular_gfx::shader(&gpu.device, "sound backdrop", include_str!("blur.wgsl"));
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sound backdrop"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let pipeline = specular_gfx::fullscreen_pipeline(
            gpu,
            "sound backdrop",
            &module,
            "fs_blur",
            &[Some(&layout)],
            HDR,
            None,
        );
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sound backdrop"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let step = 2.0 / SIZE as f32;
        let uniforms = [
            specular_gfx::uniform_buffer(&gpu.device, "sound backdrop", &[step, 0.0, 0.0, 0.0]),
            specular_gfx::uniform_buffer(&gpu.device, "sound backdrop", &[0.0, step, 0.0, 0.0]),
        ];
        Backdrop {
            pipeline,
            layout,
            sampler,
            uniforms,
            targets: None,
            pending: None,
        }
    }

    /// Blurs `cover` (an sRGB texture view) at the next [`Backdrop::render`]; the view to
    /// draw (new: register it again).
    pub(super) fn set_cover(&mut self, gpu: &Gpu, cover: &wgpu::TextureView) -> &wgpu::TextureView {
        let [a, b] = self
            .targets
            .get_or_insert_with(|| [target(gpu, SIZE, SIZE), target(gpu, SIZE, SIZE)]);
        let group = |i: usize, view: &wgpu::TextureView| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sound backdrop"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniforms[i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        self.pending = Some([group(0, cover), group(1, &a.1)]);
        &b.1
    }

    /// Draws a pending blur.
    pub(super) fn render(&mut self, enc: &mut wgpu::CommandEncoder) {
        let (Some(groups), Some(targets)) = (self.pending.take(), &self.targets) else {
            return;
        };
        for (i, g) in groups.iter().enumerate() {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sound backdrop"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets[i].1,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, g, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
