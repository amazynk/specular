//! Reflectance transformation images relit on the GPU (`shaders/relight.wgsl`).
//!
//! The coefficient layers (three byte planes each, [`specular_rti::Layer`]) live in an
//! Rgba8Unorm array texture; a light change only rewrites a small uniform (each layer's
//! scale, offset and 3×3 colour and normal matrices for that light, from
//! [`Rti::prepare`]) and redraws the overview with its mips, so dragging the light needs no
//! decode and no upload. The arithmetic is [`Rti::shade`]'s, in the same order, in f32; the
//! tests check the two against each other in every mode. Images larger than the GPU's
//! texture limit keep a box-downscaled copy of their coefficients ([`Rti::downscaled`]).

use bytemuck::{Pod, Zeroable};
use specular_gfx::BufferInit;
use specular_gfx::{FULLSCREEN_WGSL, Gpu, fullscreen_pipeline, wgpu};
use specular_rti::{MAX_LAYERS, Prepared, Rti};

use super::{
    GpuImage, ImageRenderer, Overview, STORE, sampled_view, tex_entry, texture_2d, uniform_entry,
};

/// Most bytes of coefficient texture an image keeps (larger images are downscaled).
const PLANE_BUDGET: u64 = 512 << 20;

/// An image's coefficient layers on the GPU.
#[derive(Debug)]
pub struct Planes {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Texture size (the stored size unless that exceeds the limit or budget).
    pub width: u32,
    pub height: u32,
    /// The box factor of a downscaled copy (1 at full size).
    pub factor: u32,
    /// Byte-to-value mapping of each layer.
    mapping: Vec<([f32; 3], [f32; 3])>,
}

impl Planes {
    pub fn bytes(&self) -> u64 {
        self.width as u64 * self.height as u64 * 4 * self.mapping.len() as u64
    }
}

pub struct Pipes {
    pipe: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LayerU {
    scale: [f32; 4],
    offset: [f32; 4],
    colour: [[f32; 4]; 3],
    normal: [[f32; 4]; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RelightU {
    colour: [f32; 4],
    normal: [f32; 4],
    light: [f32; 4],
    params: [f32; 4],
    flags: [u32; 4],
    layers: [LayerU; MAX_LAYERS],
}

fn v4(v: [f32; 3]) -> [f32; 4] {
    [v[0], v[1], v[2], 0.0]
}

impl Pipes {
    fn new(gpu: &Gpu) -> Pipes {
        let d = &gpu.device;
        let module = specular_gfx::shader(
            d,
            "relight",
            &format!(
                "{FULLSCREEN_WGSL}\n{}",
                include_str!("../shaders/relight.wgsl")
            ),
        );
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("relight"),
            entries: &[
                uniform_entry(0),
                tex_entry(1, wgpu::TextureViewDimension::D2Array, false),
            ],
        });
        let pipe = fullscreen_pipeline(
            gpu,
            "relight",
            &module,
            "fs_relight",
            &[Some(&bgl)],
            STORE,
            None,
        );
        Pipes { pipe, bgl }
    }
}

/// The texture size for coefficients of `w × h` with `layers` layers: the integer box
/// factor that fits the texture limit and [`PLANE_BUDGET`].
pub fn plane_factor(w: u32, h: u32, layers: usize, max_tex: u32) -> u32 {
    let mut f = 1u32;
    loop {
        let (tw, th) = (w.div_ceil(f), h.div_ceil(f));
        if tw <= max_tex
            && th <= max_tex
            && tw as u64 * th as u64 * 4 * layers as u64 <= PLANE_BUDGET
        {
            return f;
        }
        f += 1;
    }
}

impl ImageRenderer {
    /// Uploads a reflectance transformation image's coefficients for [`Self::relight`]
    /// (replacing any earlier ones). Returns false when it has more layers than the shader
    /// takes or the GPU allows.
    pub fn set_relight(&mut self, gpu: &Gpu, img: &mut GpuImage, rti: &Rti) -> bool {
        let layers = rti.layers.len();
        if layers == 0 || layers > MAX_LAYERS || layers as u32 > gpu.limits.max_texture_array_layers
        {
            return false;
        }
        let f = plane_factor(rti.width, rti.height, layers, self.max_tex);
        let small;
        let rti = if f > 1 {
            small = rti.downscaled(f);
            &small
        } else {
            rti
        };
        let (w, h) = (rti.width, rti.height);
        let texture = texture_2d(
            &gpu.device,
            "relight planes",
            w,
            h,
            layers as u32,
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let mut rgba = vec![255u8; w as usize * h as usize * 4];
        for (j, l) in rti.layers.iter().enumerate() {
            for (o, i) in rgba.chunks_exact_mut(4).zip(l.data.chunks_exact(3)) {
                o[..3].copy_from_slice(i);
            }
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: j as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        img.relight = Some(Planes {
            texture,
            view,
            width: w,
            height: h,
            factor: f,
            mapping: rti.layers.iter().map(|l| (l.scale, l.offset)).collect(),
        });
        true
    }

    /// Redraws the overview of an image with coefficients ([`Self::set_relight`]) for a
    /// look, mips included. Returns false (drawing nothing) without coefficients or when the
    /// look was prepared for another layer count.
    pub fn relight(&mut self, gpu: &Gpu, img: &mut GpuImage, p: &Prepared) -> bool {
        let Some(planes) = img.relight.as_ref() else {
            return false;
        };
        let n = planes.mapping.len();
        if p.colour.matrices.len() != n || p.normal.matrices.len() != n {
            return false;
        }
        let t0 = std::time::Instant::now();
        let (w, h, factor) = (planes.width, planes.height, planes.factor);
        let mut u = RelightU::zeroed();
        u.colour = v4(p.colour.constant);
        u.normal = v4(p.normal.constant);
        u.light = v4(p.light);
        u.params = [p.gain, p.kd, p.ks, p.exponent];
        u.flags = [p.mode.index(), p.albedo as u32, n as u32, 0];
        for (j, ((scale, offset), l)) in planes.mapping.iter().zip(&mut u.layers).enumerate() {
            l.scale = v4(*scale);
            l.offset = v4(*offset);
            l.colour = p.colour.matrices[j].map(v4);
            l.normal = p.normal.matrices[j].map(v4);
        }
        let d = &gpu.device;
        let ubuf = d.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("relight"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let mips = 32 - w.max(h).leading_zeros();
        // Draw over the current overview when it has the size (the light is being dragged).
        let reuse = img
            .overview
            .take_if(|o| (o.width, o.height, o.mips, o.texture.format()) == (w, h, mips, STORE));
        let texture = match reuse {
            Some(o) => o.texture,
            None => self.texture(
                d,
                "overview",
                w,
                h,
                mips,
                STORE,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
        };
        let Some(planes) = img.relight.as_ref() else {
            return false;
        };
        let pipes = self.relight.get_or_insert_with(|| Pipes::new(gpu));
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("relight"),
            layout: &pipes.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubuf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&planes.view),
                },
            ],
        });
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("relight"),
        });
        {
            let v0 = texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: 0,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("relight"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &v0,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            rp.set_pipeline(&pipes.pipe);
            rp.set_bind_group(0, &bind, &[]);
            rp.draw(0..3, 0..1);
        }
        self.record_mips(gpu, &mut enc, &texture, mips, false);
        gpu.queue.submit([enc.finish()]);
        let view = sampled_view(&texture, Default::default());
        let old = img.overview.replace(Overview {
            texture,
            view,
            width: w,
            height: h,
            mips,
            source_scale: factor,
        });
        if let Some(o) = old {
            self.recycle(o.texture);
        }
        img.hdr = false;
        img.content_peak = 1.0;
        img.revision += 1;
        self.last_upload_ms = t0.elapsed().as_secs_f64() * 1000.0;
        true
    }
}

impl GpuImage {
    /// Whether the image has coefficients to relight ([`ImageRenderer::set_relight`]).
    pub fn is_relightable(&self) -> bool {
        self.relight.is_some()
    }

    pub(super) fn relight_bytes(&self) -> u64 {
        self.relight.as_ref().map_or(0, Planes::bytes)
    }

    pub(super) fn take_relight_texture(&mut self) -> Option<wgpu::Texture> {
        self.relight.take().map(|p| p.texture)
    }
}

#[cfg(test)]
#[path = "relight_tests.rs"]
mod tests;
