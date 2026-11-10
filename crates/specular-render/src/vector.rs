//! Drawings on the GPU: a prepared vector sheet ([`specular_vector::prep`]) rendered by
//! vello at the exact view (every zoom is sharp, there is no pixel pyramid to run out of),
//! into a texture of the viewport's size, then blended over the scene target where the
//! CPU-rasterised picture of the sheet was drawn.
//!
//! Paths are mapped to window pixels in `f64` before vello sees them (it works in `f32`),
//! so a corner of a large drawing zoomed far in stays exact. Creating the renderer
//! compiles vello's shaders, which takes a while: [`VectorRenderer::new`] is meant for a
//! background thread.

use std::time::Instant;

use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, shader, wgpu};
use specular_vector::draw::{self, LayerStyle, Sink, Stats, View};
use specular_vector::kurbo::{self, Affine, BezPath, PathEl, Point, Rect};
use specular_vector::prep::{PathRef, PreparedSheet};
use vello::peniko::{self, Fill};

const BLIT_WGSL: &str = r"
@group(0) @binding(0) var src: texture_2d<f32>;

@fragment
fn fs_blit(i: VOut) -> @location(0) vec4<f32> {
    let c = textureLoad(src, vec2<i32>(i.pos.xy), 0);
    return vec4<f32>(c.rgb * c.a, c.a);
}
";

/// What one render took and drew.
#[derive(Clone, Copy, Debug, Default)]
pub struct VectorStats {
    pub walk: Stats,
    /// Building vello's scene (CPU) and submitting it, milliseconds.
    pub encode_ms: f64,
    pub submit_ms: f64,
}

struct Target {
    size: (u32, u32),
    /// Written by vello (storage, `Rgba8Unorm`), read by the blit as sRGB.
    storage: wgpu::TextureView,
    bind: wgpu::BindGroup,
    _texture: wgpu::Texture,
}

/// vello and the blit onto the scene target.
pub struct VectorRenderer {
    renderer: vello::Renderer,
    scene: vello::Scene,
    target: Option<Target>,
    pipe: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    /// Whether the texture holds a picture to blit.
    drawn: bool,
}

impl std::fmt::Debug for VectorRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorRenderer")
            .field("size", &self.target.as_ref().map(|t| t.size))
            .finish()
    }
}

impl VectorRenderer {
    /// Compiles vello's pipelines (slow: run it off the UI thread).
    pub fn new(gpu: &Gpu) -> Result<VectorRenderer, String> {
        let renderer = vello::Renderer::new(
            &gpu.device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                // vello's own threads would be unnamed; this runs on a named one already.
                num_init_threads: std::num::NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .map_err(|e| e.to_string())?;
        let d = &gpu.device;
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vector blit"),
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
        let module = shader(d, "vector blit", &format!("{FULLSCREEN_WGSL}\n{BLIT_WGSL}"));
        let pipe = fullscreen_pipeline(
            gpu,
            "vector blit",
            &module,
            "fs_blit",
            &[Some(&bgl)],
            HDR,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        Ok(VectorRenderer {
            renderer,
            scene: vello::Scene::new(),
            target: None,
            pipe,
            bgl,
            drawn: false,
        })
    }

    fn ensure_target(&mut self, gpu: &Gpu, size: (u32, u32)) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vector"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });
        let storage = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            usage: Some(wgpu::TextureUsages::STORAGE_BINDING),
            ..Default::default()
        });
        // An sRGB view can only be sampled (sRGB formats have no storage).
        let srgb = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
            ..Default::default()
        });
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vector blit"),
            layout: &self.bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&srgb),
            }],
        });
        self.target = Some(Target {
            size,
            storage,
            bind,
            _texture: texture,
        });
    }

    /// Renders `sheet` for `view` into the renderer's texture: the background over `paper`
    /// (window pixels, the sheet's picture), then the drawing clipped to it. Submits its
    /// own GPU work; [`VectorRenderer::blit`] puts the result on the scene.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        sheet: &PreparedSheet,
        view: &View<'_>,
        paper: Rect,
    ) -> Result<VectorStats, String> {
        self.render_styled(gpu, sheet, view, &[], paper, true)
    }

    /// [`VectorRenderer::render`] with layer `styles` over the sheet's own
    /// ([`draw::draw_styled`]); without `background` the paper stays transparent, so the
    /// sheet lies over what the scene already shows (a map's layers over its raster).
    pub fn render_styled(
        &mut self,
        gpu: &Gpu,
        sheet: &PreparedSheet,
        view: &View<'_>,
        styles: &[LayerStyle],
        paper: Rect,
        background: bool,
    ) -> Result<VectorStats, String> {
        let t0 = Instant::now();
        self.ensure_target(gpu, view.size);
        self.scene.reset();
        let [r, g, b] = view.background;
        let bg = peniko::Color::from_rgba8(r, g, b, 255);
        let screen = Rect::new(0.0, 0.0, f64::from(view.size.0), f64::from(view.size.1));
        let paper = paper.intersect(screen);
        let mut walk = Stats::default();
        if paper.width() > 0.0 && paper.height() > 0.0 {
            if background {
                self.scene
                    .fill(Fill::NonZero, Affine::IDENTITY, bg, None, &paper);
            }
            self.scene
                .push_clip_layer(Fill::NonZero, Affine::IDENTITY, &paper);
            let mut sink = VelloSink {
                scene: &mut self.scene,
                clips: 0,
                bounds: paper,
            };
            walk = draw::draw_styled(sheet, view, styles, &mut sink);
            for _ in 0..sink.clips {
                self.scene.pop_layer();
            }
            self.scene.pop_layer();
        }
        let encode_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        let Some(target) = &self.target else {
            return Err("no target".into());
        };
        self.renderer
            .render_to_texture(
                &gpu.device,
                &gpu.queue,
                &self.scene,
                &target.storage,
                &vello::RenderParams {
                    base_color: peniko::Color::TRANSPARENT,
                    width: view.size.0.max(1),
                    height: view.size.1.max(1),
                    antialiasing_method: vello::AaConfig::Area,
                },
            )
            .map_err(|e| e.to_string())?;
        self.drawn = true;
        Ok(VectorStats {
            walk,
            encode_ms,
            submit_ms: t1.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// Forgets the last picture (nothing is blitted until the next render).
    pub fn clear(&mut self) {
        self.drawn = false;
    }

    /// Blends the last render over `target` (the scene, premultiplied linear light).
    pub fn blit(&self, enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let Some(t) = self.target.as_ref().filter(|_| self.drawn) else {
            return;
        };
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("vector blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&self.pipe);
        rp.set_bind_group(0, &t.bind, &[]);
        rp.draw(0..3, 0..1);
    }
}

/// A path mapped to window pixels in `f64`.
struct Mapped<'a> {
    path: PathRef<'a>,
    xf: Affine,
}

impl kurbo::Shape for Mapped<'_> {
    type PathElementsIter<'iter>
        = std::iter::Map<
        specular_vector::prep::PathIter<'iter>,
        Box<dyn Fn(PathEl) -> PathEl + 'iter>,
    >
    where
        Self: 'iter;

    fn path_elements(&self, _tolerance: f64) -> Self::PathElementsIter<'_> {
        let xf = self.xf;
        self.path.iter().map(Box::new(move |e| xf * e))
    }
    fn area(&self) -> f64 {
        (self.xf * self.path.to_bez()).area()
    }
    fn perimeter(&self, accuracy: f64) -> f64 {
        (self.xf * self.path.to_bez()).perimeter(accuracy)
    }
    fn winding(&self, pt: Point) -> i32 {
        (self.xf * self.path.to_bez()).winding(pt)
    }
    fn bounding_box(&self) -> Rect {
        self.xf
            .transform_rect_bbox(self.path.to_bez().bounding_box())
    }
}

struct VelloSink<'a> {
    scene: &'a mut vello::Scene,
    /// Clip, group and erase layers open (closed in reverse at the end).
    clips: usize,
    /// The pixels drawn into: the extent of group and erase layers.
    bounds: Rect,
}

fn color(c: [u8; 4]) -> peniko::Color {
    peniko::Color::from_rgba8(c[0], c[1], c[2], c[3])
}

impl Sink for VelloSink<'_> {
    fn stroke(&mut self, path: PathRef<'_>, transform: Affine, width: f64, c: [u8; 4]) {
        let px = width * transform.determinant().abs().sqrt();
        let style = kurbo::Stroke::new(px)
            .with_caps(kurbo::Cap::Round)
            .with_join(kurbo::Join::Round);
        self.scene.stroke(
            &style,
            Affine::IDENTITY,
            color(c),
            None,
            &Mapped {
                path,
                xf: transform,
            },
        );
    }

    fn fill(&mut self, path: PathRef<'_>, transform: Affine, even_odd: bool, c: [u8; 4]) {
        let rule = if even_odd {
            Fill::EvenOdd
        } else {
            Fill::NonZero
        };
        self.scene.fill(
            rule,
            Affine::IDENTITY,
            color(c),
            None,
            &Mapped {
                path,
                xf: transform,
            },
        );
    }

    fn glyph(&mut self, path: &BezPath, transform: Affine, c: [u8; 4]) {
        self.scene
            .fill(Fill::NonZero, transform, color(c), None, path);
    }

    fn dot(&mut self, center: Point, radius: f64, c: [u8; 4]) {
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            color(c),
            None,
            &kurbo::Circle::new(center, radius.max(0.5)),
        );
    }

    fn push_clip(&mut self, clip: &BezPath, transform: Affine) {
        self.scene
            .push_clip_layer(Fill::NonZero, Affine::IDENTITY, &(transform * clip.clone()));
        self.clips += 1;
    }

    fn pop_clip(&mut self) {
        if self.clips > 0 {
            self.scene.pop_layer();
            self.clips -= 1;
        }
    }

    fn push_group(&mut self, opacity: f32) {
        self.scene.push_layer(
            Fill::NonZero,
            peniko::BlendMode::default(),
            opacity,
            Affine::IDENTITY,
            &self.bounds,
        );
        self.clips += 1;
    }

    fn pop_group(&mut self) {
        self.pop_clip();
    }

    fn set_erase(&mut self, erase: bool) {
        if erase {
            self.scene.push_layer(
                Fill::NonZero,
                peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::DestOut),
                1.0,
                Affine::IDENTITY,
                &self.bounds,
            );
            self.clips += 1;
        } else {
            self.pop_clip();
        }
    }
}
