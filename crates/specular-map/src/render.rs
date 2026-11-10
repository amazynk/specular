//! Draws the base map ([`World`]) for a [`Camera`] into a texture of its own (the UI's HDR
//! format, linear premultiplied), which the app then shows as a UI image under the map's
//! markers and chrome.
//!
//! Land is filled with the stencil trick: every land and lake ring is drawn as a triangle fan
//! that inverts the stencil, so a pixel ends up odd where it is inside land and not inside a
//! lake; one full-screen pass then paints the land colour over the odd pixels. Coastlines
//! (the rings' outlines, without the cuts along the antimeridian and the bottom edge) and
//! borders are instanced quads. 4× MSAA where the format allows it. The world repeats east–
//! west: each copy in view is one more draw with another uniform offset.

use specular_gfx::BufferInit;
use specular_gfx::wgpu;
use specular_gfx::{Gpu, HDR, shader};

use crate::camera::Camera;
use crate::world::{Q, World};

/// Colours (linear, premultiplied) and line widths (logical px) of the base map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub ocean: [f32; 4],
    pub land: [f32; 4],
    pub coast: [f32; 4],
    pub border: [f32; 4],
    pub coast_px: f32,
    pub border_px: f32,
}

const STENCIL: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
/// World copies drawn at most (a very wide window at the least zoom).
const COPIES: usize = 8;
const VIEW_SIZE: u64 = 32;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewU {
    c: [i32; 2],
    f: [f32; 2],
    k: [f32; 2],
    px: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StyleU {
    land: [f32; 4],
    coast: [f32; 4],
    border: [f32; 4],
    widths: [f32; 4],
}

struct Targets {
    size: (u32, u32),
    /// The multisampled colour target (None without MSAA: then `resolved` is drawn into).
    msaa: Option<wgpu::TextureView>,
    stencil: wgpu::TextureView,
    resolved: wgpu::TextureView,
}

pub struct MapRenderer {
    samples: u32,
    fill: wgpu::RenderPipeline,
    cover: wgpu::RenderPipeline,
    coast: wgpu::RenderPipeline,
    border: wgpu::RenderPipeline,
    points: wgpu::Buffer,
    fans: wgpu::Buffer,
    fan_count: u32,
    coast_segs: wgpu::Buffer,
    coast_count: u32,
    border_segs: wgpu::Buffer,
    border_count: u32,
    views: wgpu::Buffer,
    stride: u64,
    style: wgpu::Buffer,
    bind: wgpu::BindGroup,
    targets: Option<Targets>,
}

impl std::fmt::Debug for MapRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapRenderer")
            .field("samples", &self.samples)
            .field("size", &self.targets.as_ref().map(|t| t.size))
            .finish()
    }
}

/// Triangle-fan indices of every land and lake ring.
fn fans(w: &World) -> Vec<u32> {
    let mut out = Vec::new();
    for r in w.land.iter().chain(&w.lakes) {
        for i in r.start + 1..r.end - 1 {
            out.extend([r.start, i, i + 1]);
        }
    }
    out
}

/// Coastline segments: ring edges, except those running along the world's edges (where
/// Natural Earth cuts land at the antimeridian and closes Antarctica along the bottom).
fn coast_segments(w: &World) -> Vec<[u32; 4]> {
    let along = |a: [u32; 2], b: [u32; 2]| {
        (a[0] == b[0] && (a[0] == 0 || a[0] == Q)) || (a[1] == b[1] && (a[1] == 0 || a[1] == Q))
    };
    let mut out = Vec::new();
    for r in w.land.iter().chain(&w.lakes) {
        let p = w.of(r);
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            if !along(a, b) {
                out.push([a[0], a[1], b[0], b[1]]);
            }
        }
    }
    out
}

fn line_segments(w: &World) -> Vec<[u32; 4]> {
    w.borders
        .iter()
        .flat_map(|r| {
            w.of(r)
                .windows(2)
                .map(|s| [s[0][0], s[0][1], s[1][0], s[1][1]])
        })
        .collect()
}

impl MapRenderer {
    pub fn new(gpu: &Gpu) -> MapRenderer {
        let d = &gpu.device;
        let world = World::get();
        let samples = if gpu
            .adapter
            .get_texture_format_features(HDR)
            .flags
            .sample_count_supported(4)
            && gpu
                .adapter
                .get_texture_format_features(STENCIL)
                .flags
                .sample_count_supported(4)
        {
            4
        } else {
            1
        };
        let module = shader(d, "map", include_str!("map.wgsl"));
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(VIEW_SIZE),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("map"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let stride = u64::from(gpu.limits.min_uniform_buffer_offset_alignment).max(VIEW_SIZE);
        let views = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map views"),
            size: stride * COPIES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let style = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map style"),
            size: std::mem::size_of::<StyleU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("map"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &views,
                        offset: 0,
                        size: wgpu::BufferSize::new(VIEW_SIZE),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: style.as_entire_binding(),
                },
            ],
        });

        let stencil = |compare, pass_op, write| wgpu::DepthStencilState {
            format: STENCIL,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op,
                },
                back: wgpu::StencilFaceState {
                    compare,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op,
                },
                read_mask: 0xff,
                write_mask: write,
            },
            bias: wgpu::DepthBiasState::default(),
        };
        let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let pipe = |label: &str,
                    vs: &str,
                    fs: &str,
                    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                    ds: wgpu::DepthStencilState,
                    writes: wgpu::ColorWrites| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(ds),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR,
                        blend,
                        write_mask: writes,
                    })],
                }),
                multiview_mask: None,
                cache: gpu.cache(),
            })
        };
        let point_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Uint32x2],
        };
        let seg_layout = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Uint32x4],
        };
        use wgpu::CompareFunction as C;
        use wgpu::StencilOperation as S;
        let fill = pipe(
            "map fill",
            "vs_fill",
            "fs_none",
            &[Some(point_layout)],
            stencil(C::Always, S::Invert, 1),
            wgpu::ColorWrites::empty(),
        );
        let cover = pipe(
            "map land",
            "vs_cover",
            "fs_land",
            &[],
            stencil(C::NotEqual, S::Zero, 0xff),
            wgpu::ColorWrites::ALL,
        );
        let coast = pipe(
            "map coast",
            "vs_coast",
            "fs_line",
            &[Some(seg_layout.clone())],
            stencil(C::Always, S::Keep, 0),
            wgpu::ColorWrites::ALL,
        );
        let border = pipe(
            "map border",
            "vs_border",
            "fs_line",
            &[Some(seg_layout.clone())],
            stencil(C::Always, S::Keep, 0),
            wgpu::ColorWrites::ALL,
        );

        let buf = |label: &str, data: &[u8], usage| {
            d.buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: data,
                usage,
            })
        };
        let fan = fans(world);
        let cs = coast_segments(world);
        let bs = line_segments(world);
        MapRenderer {
            samples,
            fill,
            cover,
            coast,
            border,
            points: buf(
                "map points",
                bytemuck::cast_slice(&world.points),
                wgpu::BufferUsages::VERTEX,
            ),
            fans: buf(
                "map fans",
                bytemuck::cast_slice(&fan),
                wgpu::BufferUsages::INDEX,
            ),
            fan_count: fan.len() as u32,
            coast_segs: buf(
                "map coast",
                bytemuck::cast_slice(&cs),
                wgpu::BufferUsages::VERTEX,
            ),
            coast_count: cs.len() as u32,
            border_segs: buf(
                "map borders",
                bytemuck::cast_slice(&bs),
                wgpu::BufferUsages::VERTEX,
            ),
            border_count: bs.len() as u32,
            views,
            stride,
            style,
            bind,
            targets: None,
        }
    }

    /// Makes the target `px` (physical px) large; true when it was (re)made, so the texture
    /// must be registered with the UI again.
    pub fn ensure_target(&mut self, gpu: &Gpu, px: (u32, u32)) -> bool {
        let px = (px.0.max(1), px.1.max(1));
        if self.targets.as_ref().is_some_and(|t| t.size == px) {
            return false;
        }
        let tex = |label: &str, format, samples, usage| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: px.0,
                        height: px.1,
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
        let ra = wgpu::TextureUsages::RENDER_ATTACHMENT;
        self.targets = Some(Targets {
            size: px,
            msaa: (self.samples > 1).then(|| tex("map msaa", HDR, self.samples, ra)),
            stencil: tex("map stencil", STENCIL, self.samples, ra),
            resolved: tex("map", HDR, 1, ra | wgpu::TextureUsages::TEXTURE_BINDING),
        });
        true
    }

    /// The finished map (after [`MapRenderer::render`]), for the UI.
    pub fn view(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|t| &t.resolved)
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        self.targets.as_ref().map(|t| t.size)
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Encodes the base map for `cam` (whose size is in logical px; `scale` physical px per
    /// logical px) into the target.
    pub fn render(
        &self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        cam: &Camera,
        scale: f64,
        style: &Style,
    ) {
        let Some(t) = &self.targets else { return };
        let (w, h) = (f64::from(t.size.0), f64::from(t.size.1));
        // Physical px per quantised unit, then NDC per unit.
        let s = cam.scale() * scale / f64::from(Q);
        let (cx, cy) = (cam.center.0 * f64::from(Q), cam.center.1 * f64::from(Q));
        let (ci, cf) = ((cx.floor(), cy.floor()), (cx - cx.floor(), cy - cy.floor()));
        let (x0, _, x1, _) = cam.visible();
        let first = x0.floor() as i64;
        let copies: Vec<i64> = (first..=x1.floor() as i64).take(COPIES).collect();
        let mut data = vec![0u8; (self.stride * COPIES as u64) as usize];
        for (slot, &k) in copies.iter().enumerate() {
            let v = ViewU {
                c: [(ci.0 as i64 - k * i64::from(Q)) as i32, ci.1 as i32],
                f: [cf.0 as f32, cf.1 as f32],
                k: [(2.0 * s / w) as f32, (-2.0 * s / h) as f32],
                px: [(2.0 / w) as f32, (-2.0 / h) as f32],
            };
            let at = slot * self.stride as usize;
            data[at..at + VIEW_SIZE as usize].copy_from_slice(bytemuck::bytes_of(&v));
        }
        gpu.queue.write_buffer(&self.views, 0, &data);
        let sc = scale as f32;
        let su = StyleU {
            land: style.land,
            coast: style.coast,
            border: style.border,
            widths: [style.coast_px * sc, style.border_px * sc, 0.0, 0.0],
        };
        gpu.queue
            .write_buffer(&self.style, 0, bytemuck::bytes_of(&su));

        let o = style.ocean;
        let (view, resolve) = match &t.msaa {
            Some(m) => (m, Some(&t.resolved)),
            None => (&t.resolved, None),
        };
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("map"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(o[0]),
                        g: f64::from(o[1]),
                        b: f64::from(o[2]),
                        a: f64::from(o[3]),
                    }),
                    store: if resolve.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.stencil,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Discard,
                }),
            }),
            ..Default::default()
        });
        let offset = |slot: usize| (slot as u64 * self.stride) as u32;
        rp.set_pipeline(&self.fill);
        rp.set_vertex_buffer(0, self.points.slice(..));
        rp.set_index_buffer(self.fans.slice(..), wgpu::IndexFormat::Uint32);
        for slot in 0..copies.len() {
            rp.set_bind_group(0, &self.bind, &[offset(slot)]);
            rp.draw_indexed(0..self.fan_count, 0, 0..1);
        }
        rp.set_pipeline(&self.cover);
        rp.set_stencil_reference(0);
        rp.set_bind_group(0, &self.bind, &[0]);
        rp.draw(0..3, 0..1);
        for (pipe, buf, n) in [
            (&self.border, &self.border_segs, self.border_count),
            (&self.coast, &self.coast_segs, self.coast_count),
        ] {
            rp.set_pipeline(pipe);
            rp.set_vertex_buffer(0, buf.slice(..));
            for slot in 0..copies.len() {
                rp.set_bind_group(0, &self.bind, &[offset(slot)]);
                rp.draw(0..6, 0..n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_of_the_built_in_map() {
        let w = World::get();
        let f = fans(w);
        let rings: u32 = w
            .land
            .iter()
            .chain(&w.lakes)
            .map(|r| r.end - r.start - 2)
            .sum();
        assert_eq!(f.len() as u32, rings * 3);
        let c = coast_segments(w);
        let edges: usize = w.land.iter().chain(&w.lakes).map(|r| w.of(r).len()).sum();
        // Only a handful of edges run along the world's edges.
        assert!(
            c.len() < edges && c.len() > edges - 500,
            "{} of {edges}",
            c.len()
        );
        assert!(
            c.iter()
                .all(|s| !(s[0] == s[2] && (s[0] == 0 || s[0] == Q)))
        );
        assert!(!line_segments(w).is_empty());
    }
}
