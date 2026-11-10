//! The slideshow's cross-fade (DESIGN.md §6.5): the outgoing scene is kept in a texture of
//! its own and blended over the incoming one in the composite, before the chrome is drawn.
//! In linear light, like the rest of the composite, so the midpoint is the average of the two
//! images and not a dark dip.

use crate::compose::{Compositor, FULLSCREEN_WGSL, HDR, Target, fullscreen_pipeline};
use crate::gpu::{Gpu, shader};

const FADE_WGSL: &str = r"
@group(0) @binding(0) var prev: texture_2d<f32>;

@fragment
fn fs_fade(i: VOut) -> @location(0) vec4<f32> {
    return textureLoad(prev, vec2<i32>(i.pos.xy), 0);
}
";

pub struct Crossfade {
    pipe: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    /// The outgoing scene and its bind group, while a fade runs.
    prev: Option<(Target, wgpu::BindGroup)>,
}

impl std::fmt::Debug for Crossfade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Crossfade")
            .field(
                "snapshot",
                &self.prev.as_ref().map(|p| (p.0.width, p.0.height)),
            )
            .finish()
    }
}

impl Crossfade {
    pub fn new(gpu: &Gpu) -> Crossfade {
        let d = &gpu.device;
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("crossfade"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let module = shader(d, "crossfade", &format!("{FULLSCREEN_WGSL}\n{FADE_WGSL}"));
        // out = prev · k + comp · (1 − k), k the blend constant.
        let mix = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Constant,
            dst_factor: wgpu::BlendFactor::OneMinusConstant,
            operation: wgpu::BlendOperation::Add,
        };
        let pipe = fullscreen_pipeline(
            gpu,
            "crossfade",
            &module,
            "fs_fade",
            &[Some(&bgl)],
            HDR,
            Some(wgpu::BlendState {
                color: mix,
                alpha: mix,
            }),
        );
        Crossfade {
            pipe,
            bgl,
            prev: None,
        }
    }

    /// Keeps the scene as it is now (the outgoing slide). Encoded before the scene is drawn
    /// anew.
    pub fn snapshot(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, comp: &Compositor) {
        let scene = &comp.scene;
        if self
            .prev
            .as_ref()
            .is_none_or(|p| (p.0.width, p.0.height) != (scene.width, scene.height))
        {
            let t = Target::new(
                &gpu.device,
                "crossfade",
                scene.width,
                scene.height,
                HDR,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("crossfade"),
                layout: &self.bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&t.view),
                }],
            });
            self.prev = Some((t, bind));
        }
        let Some((t, _)) = &self.prev else { return };
        enc.copy_texture_to_texture(
            scene.texture.as_image_copy(),
            t.texture.as_image_copy(),
            wgpu::Extent3d {
                width: scene.width,
                height: scene.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Whether a snapshot of the composite's size is held.
    pub fn ready(&self, comp: &Compositor) -> bool {
        self.prev
            .as_ref()
            .is_some_and(|p| (p.0.width, p.0.height) == comp.size())
    }

    /// Blends the outgoing slide over the composite with weight `k` (1: only the outgoing
    /// slide, 0: none of it). Encoded after [`Compositor::begin`], before the chrome.
    pub fn draw(&self, enc: &mut wgpu::CommandEncoder, comp: &Compositor, k: f32) {
        let Some((_, bind)) = self.prev.as_ref().filter(|_| self.ready(comp)) else {
            return;
        };
        let k = f64::from(k.clamp(0.0, 1.0));
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("crossfade"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &comp.comp.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        rp.set_pipeline(&self.pipe);
        rp.set_bind_group(0, bind, &[]);
        rp.set_blend_constant(wgpu::Color {
            r: k,
            g: k,
            b: k,
            a: k,
        });
        rp.draw(0..3, 0..1);
    }

    /// Frees the snapshot (the fade is over, or the slideshow stopped); the next
    /// [`Crossfade::snapshot`] allocates a new one. The pipeline stays.
    pub fn release(&mut self) {
        self.prev = None;
    }

    /// Whether a snapshot is held (of any size).
    pub fn holds_snapshot(&self) -> bool {
        self.prev.is_some()
    }
}
