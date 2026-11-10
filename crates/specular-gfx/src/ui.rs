//! Draws a [`DrawList`] with the instanced uber-shader.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::compose::{Compositor, HDR, uniform_buffer};
use crate::draw::{DrawList, Prim, TexId};
use crate::gpu::{Gpu, shader};
use crate::text::{ATLAS_SIZE, Text};

/// The next texture id: unique in the process, so an id kept from a renderer made before a
/// GPU reset never names a texture of the new one (it draws nothing until made again).
static NEXT_TEX: AtomicU32 = AtomicU32::new(1);

pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bgl0: wgpu::BindGroupLayout,
    bgl1: wgpu::BindGroupLayout,
    bind0: wgpu::BindGroup,
    textures: HashMap<TexId, wgpu::BindGroup>,
    dummy: wgpu::BindGroup,
    instances: wgpu::Buffer,
    capacity: usize,
    atlas: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Stages the per-frame writes (globals, instances, glyph rows).
    /// `queue.write_buffer` makes a fresh staging buffer per call, which
    /// takes the device's memory-allocator lock; a helper thread allocating
    /// a new memory block (a gallery atlas page) holds that lock for 20-30
    /// ms. The belt reuses its chunks, so a steady frame never waits on it.
    belt: wgpu::util::StagingBelt,
}

/// Belt chunk: the globals and a few thousand instances. A bigger write (a
/// full glyph-atlas upload) gets a chunk of its own, kept for reuse.
const BELT_CHUNK: u64 = 256 * 1024;

impl std::fmt::Debug for UiRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiRenderer")
            .field("textures", &self.textures.len())
            .finish()
    }
}

fn tex_entry(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl UiRenderer {
    pub fn new(gpu: &Gpu, comp: &Compositor) -> UiRenderer {
        let d = &gpu.device;
        let module = shader(d, "ui", include_str!("shaders/ui.wgsl"));
        let bgl0 = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex_entry(2, false),
                tex_entry(3, true),
            ],
        });
        let bgl1 = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui image"),
            entries: &[tex_entry(0, true)],
        });
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[Some(&bgl0), Some(&bgl1)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4];
        let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Prim>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: gpu.cache(),
        });
        let globals = uniform_buffer(d, "ui globals", &[1.0, 1.0, 0.0, 0.0]);
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let atlas = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        let bind0 = Self::make_bind0(
            d,
            &bgl0,
            &globals,
            &sampler,
            &atlas_view,
            comp.backdrop_view(),
        );
        let dummy_tex = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("dummy"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy = Self::make_bind1(d, &bgl1, &dummy_tex.create_view(&Default::default()));
        let capacity = 1024;
        let instances = Self::make_instances(d, capacity);
        UiRenderer {
            pipeline,
            globals,
            bgl0,
            bgl1,
            bind0,
            textures: HashMap::new(),
            dummy,
            instances,
            capacity,
            belt: wgpu::util::StagingBelt::new(d.clone(), BELT_CHUNK),
            atlas,
            atlas_view,
            sampler,
        }
    }

    fn make_instances(d: &wgpu::Device, n: usize) -> wgpu::Buffer {
        d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui instances"),
            size: (n * std::mem::size_of::<Prim>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_bind0(
        d: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        atlas: &wgpu::TextureView,
        backdrop: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui globals"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(atlas),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(backdrop),
                },
            ],
        })
    }

    fn make_bind1(
        d: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui image"),
            layout: bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            }],
        })
    }

    /// Call after [`Compositor::resize`]: the backdrop texture was recreated.
    pub fn compositor_resized(&mut self, gpu: &Gpu, comp: &Compositor) {
        self.bind0 = Self::make_bind0(
            &gpu.device,
            &self.bgl0,
            &self.globals,
            &self.sampler,
            &self.atlas_view,
            comp.backdrop_view(),
        );
    }

    /// Makes a texture drawable with [`DrawList::image`]. Must be filterable float.
    pub fn register_texture(&mut self, gpu: &Gpu, view: &wgpu::TextureView) -> TexId {
        let id = TexId(NEXT_TEX.fetch_add(1, Ordering::Relaxed));
        self.textures
            .insert(id, Self::make_bind1(&gpu.device, &self.bgl1, view));
        id
    }

    pub fn update_texture(&mut self, gpu: &Gpu, id: TexId, view: &wgpu::TextureView) {
        self.textures
            .insert(id, Self::make_bind1(&gpu.device, &self.bgl1, view));
    }

    pub fn unregister_texture(&mut self, id: TexId) {
        self.textures.remove(&id);
    }

    /// Draws `dl` over `target` (the composite), which keeps its contents.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        dl: &DrawList,
        text: &mut Text,
    ) {
        // The previous frame's encoder was submitted, so its chunks can come
        // back once the GPU is done with them.
        self.belt.recall();
        let marks = text.place_marks(dl);
        text.atlas.upload(&mut self.belt, enc, &self.atlas);
        if dl.prims.is_empty() {
            self.belt.finish();
            return;
        }
        let globals = [size.0 as f32, size.1 as f32, 0.0, 0.0];
        let globals: &[u8] = bytemuck::cast_slice(&globals);
        if dl.prims.len() > self.capacity {
            self.capacity = dl.prims.len().next_power_of_two();
            self.instances = Self::make_instances(&gpu.device, self.capacity);
        }
        let prims: &[u8] = bytemuck::cast_slice(&dl.prims);
        let mut written = belt_write(&mut self.belt, enc, &self.globals, 0, globals)
            && belt_write(&mut self.belt, enc, &self.instances, 0, prims);
        // The marks' primitives with their atlas positions (one that found no room draws
        // nothing).
        for (i, uv) in marks {
            let mut p = dl.prims[i];
            match uv {
                Some(uv) => p.uv = uv,
                None => p.color = [0.0; 4],
            }
            let at = (i * std::mem::size_of::<crate::Prim>()) as u64;
            written = written
                && belt_write(
                    &mut self.belt,
                    enc,
                    &self.instances,
                    at,
                    bytemuck::bytes_of(&p),
                );
        }
        self.belt.finish();
        if !written {
            // A lost device (a driver reset): nothing to draw on it.
            return;
        }
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
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
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, &self.bind0, &[]);
        rp.set_vertex_buffer(0, self.instances.slice(..));
        for b in &dl.batches {
            let bind = b
                .image
                .and_then(|t| self.textures.get(&t))
                .unwrap_or(&self.dummy);
            rp.set_bind_group(1, bind, &[]);
            rp.draw(0..4, b.start..b.end);
        }
    }
}

/// `StagingBelt::write_buffer` of all of `data` at byte `offset` of `target`, without its
/// panic on a lost device (a driver reset maps nothing): false then, and nothing is written.
fn belt_write(
    belt: &mut wgpu::util::StagingBelt,
    enc: &mut wgpu::CommandEncoder,
    target: &wgpu::Buffer,
    offset: u64,
    data: &[u8],
) -> bool {
    let align = wgpu::BufferSize::new(wgpu::COPY_BUFFER_ALIGNMENT).expect("non-zero");
    let slice = belt.allocate(nonzero(data.len()), align);
    let Ok(mut range) = slice.get_mapped_range_mut() else {
        return false;
    };
    range.slice(..data.len()).copy_from_slice(data);
    drop(range);
    enc.copy_buffer_to_buffer(
        slice.buffer(),
        slice.offset(),
        target,
        offset,
        data.len() as u64,
    );
    true
}

/// A non-empty write's size (callers check emptiness first).
fn nonzero(n: usize) -> wgpu::BufferSize {
    wgpu::BufferSize::new(n as u64).expect("non-empty write")
}
