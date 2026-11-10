//! Triangle meshes on the GPU. [`MeshRenderer::upload`] puts a [`specular_mesh::Mesh`] on
//! the GPU once; [`MeshRenderer::draw`] draws it under a [`Camera`] into a rectangle of the
//! HDR scene: shaded, shaded with edges, as a wireframe or as x-ray, with a ground grid,
//! 4× multisampled into its own targets and then shown over the scene.
//!
//! Molecules ([`MoleculeGpu`]) draw with a mesh (their cartoon) as sphere and cylinder
//! impostors. Below full opacity ([`MoleculeGpu::opacity`], for streamlines) the sticks
//! draw translucent with weighted blended order-independent transparency: one extra pass
//! into an accumulation and a revealage target, depth-tested against the opaque scene but
//! not writing depth, and a full-screen composite. Sorting was the alternative, but the
//! segments number in millions and the order changes with every orbit; this needs no
//! re-upload and no per-frame CPU work, at the price of approximate (weighted) ordering
//! among the translucent layers.
//!
//! Gaussian splats ([`SplatsGpu`], [`SplatRenderer`]) are sorted back to front on the CPU when
//! the camera moves and alpha-blended as projected anisotropic Gaussians, under the same camera
//! and grid.
//!
//! Point clouds the same way: a [`PointsGpu`] follows a [`specular_points::Loading`], taking
//! a few MB a frame ([`PointsGpu::feed`]), and [`PointRenderer::draw`] draws the octree
//! leaves a view needs under a point budget as round splats, with optional eye-dome
//! lighting, under the same camera and grid.

pub mod bvh;
mod camera;
mod molecule;
pub mod nav;
mod points;
pub mod section;
mod splats;
mod upload;

#[cfg(test)]
mod nav_tests;
#[cfg(test)]
mod tests;

pub use camera::{
    Camera, FOV_Y, ORBIT_KEY, ORBIT_PER_PX, Pose, Projected, Projection, Shading, StandardView, V3,
    ZOOM_STEP, grid_step,
};
pub use molecule::{MOL_CHUNK, MoleculeGpu, molecule_source};
pub use nav::{Body, NAV_FOV, Nav, NavInput, NavMode};
pub use points::{PAGE_POINTS, PointColour, PointRenderer, PointView, PointsGpu, UPLOAD_PER_FRAME};
pub use section::{Caps, MAX_PLANES, SectionPlane, Sections};
pub use splats::{
    SPLAT_UPLOAD_PER_FRAME, SplatRenderer, SplatView, SplatsGpu, sort_back_to_front,
    splat_present_source, splats_source,
};
pub use upload::{CHUNK, MeshGpu, canonical_bounds};

use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, shader, wgpu};

use camera::{radius, scale, sub, unit};

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A drawn mesh's depth buffer (reversed depth, the view rectangle's size).
#[derive(Clone, Copy, Debug)]
pub struct SceneDepth<'a> {
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) samples: u32,
    pub(crate) size: (u32, u32),
}

/// One view of a mesh.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Target rectangle: x, y, width, height (physical px).
    pub rect: [u32; 4],
    pub camera: Camera,
    pub shading: Shading,
    pub grid: bool,
    /// Physical px per logical px (line widths follow it).
    pub scale: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FrameU {
    vp: [[f32; 4]; 4],
    q_scale: [f32; 4],
    q_offset: [f32; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    light: [f32; 4],
    params: [f32; 4],
    grid: [f32; 4],
    grid_centre: [f32; 4],
    /// The drawn box's centre and radius.
    bounds: [f32; 4],
    /// Section planes (normal, offset) and how many, with the tolerance.
    clip: [[f32; 4]; 12],
    clip_n: [f32; 4],
}

struct Targets {
    size: (u32, u32),
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    resolved: wgpu::TextureView,
    present: wgpu::BindGroup,
    /// Made on the first translucent frame at this size.
    oit: Option<OitTargets>,
}

/// Weighted blended OIT's targets: accumulation and revealage, multisampled like the
/// scene (when it is) and resolved for the composite.
struct OitTargets {
    accum_msaa: Option<wgpu::TextureView>,
    reveal_msaa: Option<wgpu::TextureView>,
    accum: wgpu::TextureView,
    reveal: wgpu::TextureView,
    composite: wgpu::BindGroup,
}

/// Accumulation: premultiplied colour times weight, and the weight.
const ACCUM: wgpu::TextureFormat = HDR;

const ACCUM_BLEND: wgpu::BlendState = ADDITIVE;

/// Revealage: the product of (1 - alpha) over the layers.
const REVEAL_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::OneMinusSrc,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::OneMinusSrc,
        operation: wgpu::BlendOperation::Add,
    },
};

/// The grid for a model's canonical box: its spacing, centre and half extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub step: f64,
    pub centre: [f64; 2],
    pub half: f64,
    pub z: f64,
}

impl Grid {
    pub fn of(b: &specular_mesh::Bounds) -> Grid {
        let step = grid_step(b);
        let c = camera::centre(b);
        let major = step * 10.0;
        let half = ((radius(b) * 3.0).max(major * 2.0) / major).ceil() * major;
        Grid {
            step,
            centre: [
                (c[0] / major).round() * major,
                (c[1] / major).round() * major,
            ],
            half,
            z: if b.is_empty() {
                0.0
            } else {
                f64::from(b.min[2])
            },
        }
    }
}

/// Draws meshes; its pipelines, layouts and offscreen targets.
pub struct MeshRenderer {
    pub(crate) chunk_layout: wgpu::BindGroupLayout,
    present_layout: wgpu::BindGroupLayout,
    shaded: wgpu::RenderPipeline,
    wire: wgpu::RenderPipeline,
    xray: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    frame_buf: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    /// The frame section caps draw under (their own quantisation).
    caps_frame_buf: wgpu::Buffer,
    caps_frame_group: wgpu::BindGroup,
    /// Section planes the next draws cut with, and how many.
    clip: ([[f32; 4]; 12], u32),
    /// The box the next draws are framed on, in place of the mesh's own.
    frame: Option<specular_mesh::Bounds>,
    present_buf: wgpu::Buffer,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) white: wgpu::TextureView,
    pub(crate) dummy: wgpu::Buffer,
    samples: u32,
    targets: Option<Targets>,
    frame_layout: wgpu::BindGroupLayout,
    pub(crate) mol_layout: wgpu::BindGroupLayout,
    /// Made on the first molecule drawn.
    mol_pipes: Option<molecule::MolPipes>,
    /// Translucent sticks and their composite, made on the first translucent frame.
    oit_pipes: Option<OitPipes>,
    /// The revealage target's format (one channel where it multisamples, else as HDR).
    reveal_format: wgpu::TextureFormat,
}

struct OitPipes {
    sticks: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

fn layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty,
        count: None,
    }
}

fn uniform() -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

fn storage() -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

fn pipeline(
    gpu: &Gpu,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    s: PipeSpec,
    samples: u32,
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(s.label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
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
            multisample: wgpu::MultisampleState {
                count: samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(s.fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR,
                    blend: s.blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: gpu.cache(),
        })
}

const PREMULTIPLIED: wgpu::BlendState = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;

const ADDITIVE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

struct PipeSpec<'a> {
    label: &'a str,
    vs: &'a str,
    fs: &'a str,
    blend: Option<wgpu::BlendState>,
    depth_write: bool,
    compare: wgpu::CompareFunction,
}

impl MeshRenderer {
    pub fn new(gpu: &Gpu) -> MeshRenderer {
        let d = &gpu.device;
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let frame_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh frame"),
            entries: &[layout_entry(0, vf, uniform())],
        });
        let vs = wgpu::ShaderStages::VERTEX;
        let chunk_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh chunk"),
            entries: &[
                layout_entry(0, vf, uniform()),
                layout_entry(1, vs, storage()),
                layout_entry(2, vs, storage()),
                layout_entry(3, vs, storage()),
                layout_entry(4, vs, storage()),
                layout_entry(5, vs, storage()),
                layout_entry(
                    6,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                layout_entry(
                    7,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let samples = if gpu
            .adapter
            .get_texture_format_features(HDR)
            .flags
            .sample_count_supported(4)
            && gpu
                .adapter
                .get_texture_format_features(DEPTH)
                .flags
                .sample_count_supported(4)
        {
            4
        } else {
            1
        };
        let reveal_format = if gpu
            .adapter
            .get_texture_format_features(wgpu::TextureFormat::R16Float)
            .flags
            .sample_count_supported(samples)
        {
            wgpu::TextureFormat::R16Float
        } else {
            HDR
        };
        let module = shader(d, "mesh", &mesh_source());
        let mesh_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh"),
            bind_group_layouts: &[Some(&frame_layout), Some(&chunk_layout)],
            immediate_size: 0,
        });
        let grid_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh grid"),
            bind_group_layouts: &[Some(&frame_layout)],
            immediate_size: 0,
        });
        let pipe =
            |layout: &wgpu::PipelineLayout, s: PipeSpec| pipeline(gpu, &module, layout, s, samples);
        use wgpu::CompareFunction as C;
        let shaded = pipe(
            &mesh_layout,
            PipeSpec {
                label: "mesh shaded",
                vs: "vs_mesh",
                fs: "fs_shaded",
                blend: None,
                depth_write: true,
                compare: C::Greater,
            },
        );
        let wire = pipe(
            &mesh_layout,
            PipeSpec {
                label: "mesh wireframe",
                vs: "vs_mesh",
                fs: "fs_wire",
                blend: Some(PREMULTIPLIED),
                depth_write: false,
                compare: C::Always,
            },
        );
        let xray = pipe(
            &mesh_layout,
            PipeSpec {
                label: "mesh x-ray",
                vs: "vs_mesh",
                fs: "fs_xray",
                blend: Some(ADDITIVE),
                depth_write: false,
                compare: C::Always,
            },
        );
        let grid = pipe(
            &grid_layout,
            PipeSpec {
                label: "mesh grid",
                vs: "vs_grid",
                fs: "fs_grid",
                blend: Some(PREMULTIPLIED),
                depth_write: false,
                compare: C::GreaterEqual,
            },
        );
        let fs = wgpu::ShaderStages::FRAGMENT;
        let present_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh present"),
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
        let present_module = shader(d, "mesh present", &present_source());
        let present = fullscreen_pipeline(
            gpu,
            "mesh present",
            &present_module,
            "fs_present",
            &[Some(&present_layout)],
            HDR,
            Some(PREMULTIPLIED),
        );
        let frame_buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh frame"),
            size: size_of::<FrameU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });
        let caps_frame_buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("caps frame"),
            size: size_of::<FrameU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let caps_frame_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("caps frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: caps_frame_buf.as_entire_binding(),
            }],
        });
        let present_buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh present"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mesh texture"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let white_tex = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("mesh white"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let dummy = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh absent"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        MeshRenderer {
            chunk_layout,
            present_layout,
            shaded,
            wire,
            xray,
            grid,
            present,
            frame_buf,
            frame_group,
            caps_frame_buf,
            caps_frame_group,
            clip: ([[0.0; 4]; 12], 0),
            frame: None,
            present_buf,
            sampler,
            white: white_tex.create_view(&Default::default()),
            dummy,
            samples,
            targets: None,
            mol_layout: molecule::mol_layout(d),
            frame_layout,
            mol_pipes: None,
            oit_pipes: None,
            reveal_format,
        }
    }

    fn oit_pipes(&mut self, gpu: &Gpu) {
        if self.oit_pipes.is_some() {
            return;
        }
        let d = &gpu.device;
        let module = shader(d, "molecule", &molecule_source());
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("translucent sticks"),
            bind_group_layouts: &[Some(&self.frame_layout), Some(&self.mol_layout)],
            immediate_size: 0,
        });
        let sticks = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("translucent sticks"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_stick"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: self.samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_stick_oit"),
                compilation_options: Default::default(),
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: ACCUM,
                        blend: Some(ACCUM_BLEND),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: self.reveal_format,
                        blend: Some(REVEAL_BLEND),
                        write_mask: wgpu::ColorWrites::RED,
                    }),
                ],
            }),
            multiview_mask: None,
            cache: gpu.cache(),
        });
        let fs = wgpu::ShaderStages::FRAGMENT;
        let tex = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let comp_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("translucent composite"),
            entries: &[layout_entry(0, fs, tex), layout_entry(1, fs, tex)],
        });
        let comp_module = shader(d, "translucent composite", &oit_source());
        let composite = fullscreen_pipeline(
            gpu,
            "translucent composite",
            &comp_module,
            "fs_oit",
            &[Some(&comp_layout)],
            HDR,
            Some(PREMULTIPLIED),
        );
        self.oit_pipes = Some(OitPipes {
            sticks,
            composite,
            layout: comp_layout,
        });
    }

    /// Makes the translucent targets for the current size (after [`Self::targets`]).
    fn oit_targets(&mut self, gpu: &Gpu) {
        let (samples, reveal_format) = (self.samples, self.reveal_format);
        let (Some(t), Some(p)) = (self.targets.as_mut(), self.oit_pipes.as_ref()) else {
            return;
        };
        if t.oit.is_some() {
            return;
        }
        let d = &gpu.device;
        let size = t.size;
        let tex = |label, format, samples, usage| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&Default::default())
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let read = attach | wgpu::TextureUsages::TEXTURE_BINDING;
        let multi = samples > 1;
        let accum_msaa = multi.then(|| tex("oit accum msaa", ACCUM, samples, attach));
        let reveal_msaa = multi.then(|| tex("oit reveal msaa", reveal_format, samples, attach));
        let accum = tex("oit accum", ACCUM, 1, read);
        let reveal = tex("oit reveal", reveal_format, 1, read);
        let composite = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("translucent composite"),
            layout: &p.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&accum),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&reveal),
                },
            ],
        });
        t.oit = Some(OitTargets {
            accum_msaa,
            reveal_msaa,
            accum,
            reveal,
            composite,
        });
    }

    fn mol_pipes(&mut self, gpu: &Gpu) -> &molecule::MolPipes {
        if self.mol_pipes.is_none() {
            let d = &gpu.device;
            let module = shader(d, "molecule", &molecule_source());
            let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("molecule"),
                bind_group_layouts: &[Some(&self.frame_layout), Some(&self.mol_layout)],
                immediate_size: 0,
            });
            let spec = |label, vs, fs| PipeSpec {
                label,
                vs,
                fs,
                blend: None,
                depth_write: true,
                compare: wgpu::CompareFunction::Greater,
            };
            self.mol_pipes = Some(molecule::MolPipes {
                balls: pipeline(
                    gpu,
                    &module,
                    &layout,
                    spec("molecule balls", "vs_ball", "fs_ball"),
                    self.samples,
                ),
                sticks: pipeline(
                    gpu,
                    &module,
                    &layout,
                    spec("molecule sticks", "vs_stick", "fs_stick"),
                    self.samples,
                ),
            });
        }
        self.mol_pipes.as_ref().expect("made above")
    }

    /// Samples per pixel (4, or 1 where the GPU cannot multisample the targets).
    pub fn samples(&self) -> u32 {
        self.samples
    }

    fn targets(&mut self, gpu: &Gpu, size: (u32, u32)) -> &Targets {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            let d = &gpu.device;
            let tex = |label, format, samples, usage| {
                d.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
            };
            let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
            let msaa = (self.samples > 1).then(|| tex("mesh msaa", HDR, self.samples, attach));
            // Kept and readable: points drawn after the mesh are hidden behind it.
            let depth = tex(
                "mesh depth",
                DEPTH,
                self.samples,
                attach | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let resolved = tex(
                "mesh resolved",
                HDR,
                1,
                attach | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let present = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mesh present"),
                layout: &self.present_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.present_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&resolved),
                    },
                ],
            });
            self.targets = Some(Targets {
                size,
                msaa,
                depth,
                resolved,
                present,
                oit: None,
            });
        }
        self.targets.as_ref().expect("made above")
    }

    /// Draws `mesh` as `view` says into `target` (the HDR scene, `size` px) inside the
    /// view's rectangle, over what is there.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        mesh: &MeshGpu,
        view: &View,
    ) {
        self.draw_scene(gpu, enc, target, size, Some(mesh), None, view);
    }

    /// Cuts the next draws with `s` (its planes and box).
    pub fn set_sections(&mut self, s: &section::Sections) {
        self.clip = s.uniform();
    }

    /// Frames the next draws on `b` instead of the mesh's own box (a scene whose parts
    /// come and go, such as 3D Tiles, keeps one frame; points drawn with the same box line
    /// up with the mesh).
    pub fn set_frame(&mut self, b: Option<specular_mesh::Bounds>) {
        self.frame = b;
    }

    /// The depth of the last mesh drawn, for drawing points behind it hidden
    /// ([`PointRenderer::draw_over`]).
    pub fn depth(&self) -> Option<SceneDepth<'_>> {
        let t = self.targets.as_ref()?;
        Some(SceneDepth {
            view: &t.depth,
            samples: self.samples,
            size: t.size,
        })
    }

    /// [`MeshRenderer::draw`] with the section caps (uploaded by
    /// [`MeshRenderer::upload_caps`]) in the same depth buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_sectioned(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        mesh: &MeshGpu,
        caps: Option<&MeshGpu>,
        view: &View,
    ) {
        self.draw_inner(gpu, enc, target, size, Some(mesh), None, caps, view);
    }

    /// Draws a mesh, a molecule's impostors or both (a molecule and its cartoon) into one
    /// depth buffer, framed on the molecule's box when there is one.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_scene(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        mesh: Option<&MeshGpu>,
        mol: Option<&MoleculeGpu>,
        view: &View,
    ) {
        self.draw_inner(gpu, enc, target, size, mesh, mol, None, view);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_inner(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        mesh: Option<&MeshGpu>,
        mol: Option<&MoleculeGpu>,
        caps: Option<&MeshGpu>,
        view: &View,
    ) {
        let x0 = view.rect[0].min(size.0);
        let y0 = view.rect[1].min(size.1);
        let x1 = view.rect[0].saturating_add(view.rect[2]).min(size.0);
        let y1 = view.rect[1].saturating_add(view.rect[3]).min(size.1);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (w, h) = (view.rect[2].max(1), view.rect[3].max(1));
        let frame_box = mol.map(|m| &m.bounds).or(self.frame.as_ref());
        let mut frame = frame_uniform(mesh, frame_box, view, w, h);
        // Translucent sticks: drawn after everything opaque, into their own targets.
        let translucent = mol.filter(|m| m.opacity < 1.0 && m.sticks > 0);
        if let Some(m) = translucent {
            frame.params[3] = m.opacity.max(0.0);
        }
        if mol.is_some() {
            self.mol_pipes(gpu);
        }
        if translucent.is_some() {
            self.oit_pipes(gpu);
        }
        frame.clip = self.clip.0;
        frame.clip_n[0] = self.clip.1 as f32;
        gpu.queue
            .write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));
        if let Some(c) = caps {
            let mut cf = frame;
            cf.q_scale = [c.q_scale[0], c.q_scale[1], c.q_scale[2], 0.0];
            cf.q_offset = [c.q_offset[0], c.q_offset[1], c.q_offset[2], 0.0];
            cf.params[1] = 0.0;
            gpu.queue
                .write_buffer(&self.caps_frame_buf, 0, bytemuck::bytes_of(&cf));
        }
        let rect = [view.rect[0] as f32, view.rect[1] as f32, w as f32, h as f32];
        gpu.queue
            .write_buffer(&self.present_buf, 0, bytemuck::cast_slice(&rect));
        self.targets(gpu, (w, h));
        if translucent.is_some() {
            self.oit_targets(gpu);
        }
        let t = self.targets.as_ref().expect("made above");
        {
            let (view_tex, resolve) = match &t.msaa {
                Some(m) => (m, Some(&t.resolved)),
                None => (&t.resolved, None),
            };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mesh"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_tex,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: if resolve.is_some() {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        // The translucent pass and points drawn after test against it.
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.frame_group, &[]);
            if let (Some(m), Some(p), None) = (mol, &self.mol_pipes, translucent) {
                for c in m.chunks.iter().chain(&m.overlay) {
                    pass.set_bind_group(1, &c.group, &[]);
                    if c.balls > 0 {
                        pass.set_pipeline(&p.balls);
                        pass.draw(0..6, 0..c.balls);
                    }
                    if c.sticks > 0 {
                        pass.set_pipeline(&p.sticks);
                        pass.draw(0..36, 0..c.sticks);
                    }
                }
            }
            if let Some(mesh) = mesh {
                pass.set_pipeline(match view.shading {
                    Shading::Shaded | Shading::Edges => &self.shaded,
                    Shading::Wireframe => &self.wire,
                    Shading::XRay => &self.xray,
                });
                for c in &mesh.chunks {
                    pass.set_bind_group(1, &c.group, &[]);
                    pass.draw(0..c.vertices, 0..1);
                }
            }
            if let Some(caps) =
                caps.filter(|_| matches!(view.shading, Shading::Shaded | Shading::Edges))
            {
                pass.set_bind_group(0, &self.caps_frame_group, &[]);
                pass.set_pipeline(&self.shaded);
                for c in &caps.chunks {
                    pass.set_bind_group(1, &c.group, &[]);
                    pass.draw(0..c.vertices, 0..1);
                }
                pass.set_bind_group(0, &self.frame_group, &[]);
            }
            if view.grid {
                pass.set_pipeline(&self.grid);
                pass.draw(0..6, 0..1);
            }
        }
        if let (Some(m), Some(p), Some(o)) = (translucent, &self.oit_pipes, &t.oit) {
            draw_translucent(enc, &self.frame_group, t, o, p, m);
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mesh present"),
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

/// The translucent pass (sticks into accumulation and revealage, against the opaque
/// depth) and its composite over the resolved view.
fn draw_translucent(
    enc: &mut wgpu::CommandEncoder,
    frame: &wgpu::BindGroup,
    t: &Targets,
    o: &OitTargets,
    p: &OitPipes,
    m: &MoleculeGpu,
) {
    fn attach<'a>(
        msaa: &'a Option<wgpu::TextureView>,
        single: &'a wgpu::TextureView,
        clear: wgpu::Color,
    ) -> Option<wgpu::RenderPassColorAttachment<'a>> {
        let (view, resolve) = match msaa {
            Some(v) => (v, Some(single)),
            None => (single, None),
        };
        Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: resolve,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: if resolve.is_some() {
                    wgpu::StoreOp::Discard
                } else {
                    wgpu::StoreOp::Store
                },
            },
        })
    }
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("translucent sticks"),
            color_attachments: &[
                attach(&o.accum_msaa, &o.accum, wgpu::Color::TRANSPARENT),
                attach(&o.reveal_msaa, &o.reveal, wgpu::Color::WHITE),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, frame, &[]);
        pass.set_pipeline(&p.sticks);
        for c in &m.chunks {
            if c.sticks > 0 {
                pass.set_bind_group(1, &c.group, &[]);
                pass.draw(0..36, 0..c.sticks);
            }
        }
    }
    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("translucent composite"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &t.resolved,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(&p.composite);
    pass.set_bind_group(0, &o.composite, &[]);
    pass.draw(0..3, 0..1);
}

fn frame_uniform(
    mesh: Option<&MeshGpu>,
    bounds: Option<&specular_mesh::Bounds>,
    view: &View,
    w: u32,
    h: u32,
) -> FrameU {
    let b = bounds
        .or(mesh.map(|m| &m.bounds))
        .copied()
        .unwrap_or(specular_mesh::Bounds::EMPTY);
    let (mut f, _) = frame_for(&b, &view.camera, view.grid, w, h);
    if let Some(mesh) = mesh {
        let [sx, sy, sz] = mesh.q_scale;
        let [ox, oy, oz] = mesh.q_offset;
        f.q_scale = [sx, sy, sz, 0.0];
        f.q_offset = [ox, oy, oz, 0.0];
    }
    f.params = [
        1.1 * view.scale.max(0.5),
        if view.shading == Shading::Edges {
            1.0
        } else {
            0.0
        },
        xray_gain(mesh.map_or(0, |m| m.triangles)),
        0.0,
    ];
    f
}

/// The frame of a `w` × `h` px view of the box `b` (camera, light and grid; no mesh
/// quantisation or shading parameters), and the projection it came from.
pub(crate) fn frame_for(
    b: &specular_mesh::Bounds,
    cam: &Camera,
    show_grid: bool,
    w: u32,
    h: u32,
) -> (FrameU, Projected) {
    let grid = Grid::of(b);
    let depth = grid.half * std::f64::consts::SQRT_2 + radius(b);
    let p = cam.project(b, f64::from(w), f64::from(h), depth);
    let mut vp = [[0f32; 4]; 4];
    for (r, row) in p.rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            vp[c][r] = *v as f32;
        }
    }
    let f = p.forward;
    // From over the viewer's left shoulder.
    let light = unit(sub(
        sub(scale(f, -0.8), scale(cam.down, 0.6)),
        scale(cam.right, 0.35),
    ));
    let v4 = |a: V3, w: f32| [a[0] as f32, a[1] as f32, a[2] as f32, w];
    let eye = p.eye.map_or([0.0, 0.0, 0.0, 0.0], |e| v4(e, 1.0));
    let frame = FrameU {
        vp,
        q_scale: [0.0; 4],
        q_offset: [0.0; 4],
        eye,
        forward: v4(f, 0.0),
        light: v4(light, 0.3),
        params: [0.0; 4],
        grid: [
            grid.z as f32,
            grid.step as f32,
            grid.half as f32,
            if show_grid { 1.0 } else { 0.0 },
        ],
        grid_centre: [grid.centre[0] as f32, grid.centre[1] as f32, 1.0, 0.0],
        bounds: {
            let c = camera::centre(b);
            [c[0] as f32, c[1] as f32, c[2] as f32, radius(b) as f32]
        },
        clip: [[0.0; 4]; 12],
        clip_n: [0.0, (radius(b) * 2e-4) as f32, 0.0, 0.0],
    };
    (frame, p)
}

/// X-ray's per-surface opacity: lower for dense meshes, where many layers cover a pixel.
fn xray_gain(triangles: usize) -> f32 {
    let t = triangles.max(1) as f32;
    (1.4 / t.log10().max(1.0)).clamp(0.12, 0.6)
}

const FRAME_WGSL: &str = include_str!("shaders/frame.wgsl");
const GRID_WGSL: &str = include_str!("shaders/grid.wgsl");

/// The mesh shader.
pub fn mesh_source() -> String {
    format!(
        "{FRAME_WGSL}\n{}\n{GRID_WGSL}",
        include_str!("shaders/mesh.wgsl")
    )
}

/// The point shader.
pub fn points_source() -> String {
    format!(
        "{FRAME_WGSL}\n{}\n{GRID_WGSL}",
        include_str!("shaders/points.wgsl")
    )
}

/// The shader that shows the point view with eye-dome lighting.
pub fn edl_source() -> String {
    format!("{FULLSCREEN_WGSL}\n{}", include_str!("shaders/edl.wgsl"))
}

/// The shader that composites translucent sticks.
pub fn oit_source() -> String {
    format!("{FULLSCREEN_WGSL}\n{}", include_str!("shaders/oit.wgsl"))
}

/// The shader that shows the resolved view.
pub fn present_source() -> String {
    format!(
        "{FULLSCREEN_WGSL}\n{}",
        include_str!("shaders/present.wgsl")
    )
}
