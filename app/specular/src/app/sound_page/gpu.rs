//! The sound page's time-based visuals on the GPU: the analysis in textures (uploaded once
//! a sound), drawn by `sound.wgsl` into a target the size of the page, one panel a draw.

use specular_gfx::{Gpu, HDR, wgpu};
use specular_video::loudness::Report;
use specular_video::sound_view::{Analysis, BINS};

/// Texels a row of the waveform and history textures (as the shader's `ROW`).
const ROW: usize = 4096;
/// Most columns a strip of the spectrogram texture holds.
const SPEC_ROWS: usize = 4096;
/// Uniform size: 11 vec4.
const U_FLOATS: usize = 44;
/// Panels drawn at most a frame.
const PANELS: usize = 3;

/// What a panel shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Kind {
    Waveform = 0,
    Spectrogram = 1,
    History = 2,
}

/// One panel to draw this frame: where in the target (physical px), and its uniforms.
#[derive(Clone, Copy, Debug)]
pub(super) struct Panel {
    pub px: [u32; 4],
    pub u: [f32; U_FLOATS],
    /// Drawn from the detail data.
    pub detail: bool,
}

/// The panel's colours (linear premultiplied) and the part of the sound in view.
pub(super) struct Look {
    pub bg: [f32; 4],
    pub axis: [f32; 4],
    pub wave: [f32; 4],
    pub rms: [f32; 4],
    pub played: [f32; 4],
    pub played_rms: [f32; 4],
    pub mark: [f32; 4],
    pub start: f64,
    pub span: f64,
    pub played_to: Option<f64>,
}

/// The data as uploaded.
struct Data {
    cols: usize,
    lanes: usize,
    steps: usize,
    duration_s: f64,
    integrated: Option<f64>,
    _textures: [wgpu::Texture; 3],
    groups: Vec<wgpu::BindGroup>,
}

pub(super) struct SoundGpu {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniforms: Vec<wgpu::Buffer>,
    data: Option<Data>,
    /// A part of the sound analysed in more detail: its data, start and span (fractions).
    detail: Option<(Data, f64, f64)>,
    target: Option<(wgpu::Texture, wgpu::TextureView, (u32, u32))>,
    panels: Vec<Panel>,
}

fn texture(
    gpu: &Gpu,
    label: &str,
    format: wgpu::TextureFormat,
    w: usize,
    h: usize,
    bytes: &[u8],
    bpp: usize,
) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width: w.max(1) as u32,
        height: h.max(1) as u32,
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some((w.max(1) * bpp) as u32),
            rows_per_image: None,
        },
        size,
    );
    tex
}

/// `items` laid out `ROW` a row, the last row padded: (width, height, bytes).
fn rows<T: Copy>(
    items: &[T],
    pad: T,
    to_bytes: impl Fn(&[T]) -> Vec<u8>,
) -> (usize, usize, Vec<u8>) {
    let n = items.len().max(1);
    let w = n.min(ROW);
    let h = n.div_ceil(w);
    let mut all = items.to_vec();
    all.resize(w * h, pad);
    (w, h, to_bytes(&all))
}

impl SoundGpu {
    pub(super) fn new(gpu: &Gpu) -> SoundGpu {
        let module = specular_gfx::shader(&gpu.device, "sound page", include_str!("sound.wgsl"));
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sound page"),
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
                    tex_entry(1),
                    tex_entry(2),
                    tex_entry(3),
                ],
            });
        let pipeline = specular_gfx::fullscreen_pipeline(
            gpu,
            "sound page",
            &module,
            "fs_panel",
            &[Some(&layout)],
            HDR,
            None,
        );
        let uniforms = (0..PANELS)
            .map(|_| {
                specular_gfx::uniform_buffer(&gpu.device, "sound page panel", &[0.0; U_FLOATS])
            })
            .collect();
        SoundGpu {
            pipeline,
            layout,
            uniforms,
            data: None,
            detail: None,
            target: None,
            panels: Vec::new(),
        }
    }

    /// Uploads `a` (and its loudness) as the data drawn.
    pub(super) fn upload(&mut self, gpu: &Gpu, a: &Analysis, loud: Option<&Report>) {
        self.data = Some(self.make(gpu, a, loud));
        self.detail = None;
    }

    /// Uploads `a`, the analysis of the part of the sound from `from` for `span` (fractions
    /// of its length), drawn in place of the whole sound's for the waveform and spectrogram
    /// while the view lies inside it. `None` drops it.
    pub(super) fn upload_detail(&mut self, gpu: &Gpu, detail: Option<(&Analysis, f64, f64)>) {
        self.detail = detail.map(|(a, from, span)| (self.make(gpu, a, None), from, span));
    }

    fn make(&self, gpu: &Gpu, a: &Analysis, loud: Option<&Report>) -> Data {
        let f32s = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        let wave: Vec<[f32; 4]> = a.wave.iter().map(|w| [w[0], w[1], w[2], 0.0]).collect();
        let (ww, wh, wb) = rows(&wave, [0.0; 4], |v| f32s(v.as_flattened()));
        let wave_tex = texture(
            gpu,
            "sound waveform",
            wgpu::TextureFormat::Rgba32Float,
            ww,
            wh,
            &wb,
            16,
        );
        // Spectrogram: strips of BINS texels wide, SPEC_ROWS columns high.
        let cols = a.cols.max(1);
        let rows_n = cols.min(SPEC_ROWS);
        let strips = cols.div_ceil(rows_n);
        let mut sb = vec![0u8; strips * BINS * rows_n];
        for c in 0..a.cols {
            let (s, r) = (c / rows_n, c % rows_n);
            let at = r * strips * BINS + s * BINS;
            sb[at..at + BINS].copy_from_slice(&a.spec[c * BINS..(c + 1) * BINS]);
        }
        let spec_tex = texture(
            gpu,
            "sound spectrogram",
            wgpu::TextureFormat::R8Unorm,
            strips * BINS,
            rows_n,
            &sb,
            1,
        );
        let steps: Vec<[f32; 2]> = loud.map_or_else(Vec::new, |l| {
            l.momentary
                .iter()
                .zip(&l.short_term)
                .map(|(&m, &s)| [m.max(-200.0), s.max(-200.0)])
                .collect()
        });
        let (hw, hh, hb) = rows(&steps, [-200.0; 2], |v| f32s(v.as_flattened()));
        let hist_tex = texture(
            gpu,
            "sound loudness",
            wgpu::TextureFormat::Rg32Float,
            hw,
            hh,
            &hb,
            8,
        );
        let views = [&wave_tex, &spec_tex, &hist_tex].map(|t| t.create_view(&Default::default()));
        let groups = self
            .uniforms
            .iter()
            .map(|u| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("sound page"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: u.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&views[0]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&views[1]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&views[2]),
                        },
                    ],
                })
            })
            .collect();
        Data {
            cols: a.cols,
            lanes: a.lanes,
            steps: steps.len(),
            duration_s: a.duration_s,
            integrated: loud.and_then(|l| l.integrated),
            _textures: [wave_tex, spec_tex, hist_tex],
            groups,
        }
    }

    /// Lets go of the data and the target (another sound comes).
    pub(super) fn clear(&mut self) {
        self.data = None;
        self.detail = None;
        self.target = None;
        self.panels.clear();
    }

    pub(super) fn has_data(&self) -> bool {
        self.data.is_some()
    }

    /// A target of `px` (physical px); whether it is new (to register again).
    pub(super) fn ensure_target(&mut self, gpu: &Gpu, px: (u32, u32)) -> bool {
        let px = (px.0.max(1), px.1.max(1));
        if self.target.as_ref().is_some_and(|t| t.2 == px) {
            return false;
        }
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sound page"),
            size: wgpu::Extent3d {
                width: px.0,
                height: px.1,
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
        self.target = Some((tex, view, px));
        true
    }

    pub(super) fn view(&self) -> Option<&wgpu::TextureView> {
        self.target.as_ref().map(|t| &t.1)
    }

    /// Queues `kind` drawn at `px` (x, y, w, h in the target) for the next render.
    pub(super) fn panel(&mut self, kind: Kind, px: [u32; 4], look: &Look, lane_gap: f32) {
        if px[2] == 0 || px[3] == 0 || self.panels.len() >= PANELS {
            return;
        }
        // The detail, when it holds the view (the loudness history is the whole sound's).
        let detail = self.detail.as_ref().filter(|(_, from, span)| {
            kind != Kind::History
                && *from <= look.start + 1e-9
                && look.start + look.span <= from + span + 1e-9
        });
        let (d, mapped) = match (detail, &self.data) {
            (Some((d, from, span)), _) => (
                d,
                Some(Look {
                    start: (look.start - from) / span,
                    span: look.span / span,
                    played_to: look.played_to.map(|p| (p - from) / span),
                    ..*look
                }),
            ),
            (None, Some(d)) => (d, None),
            (None, None) => return,
        };
        let look = mapped.as_ref().unwrap_or(look);
        let is_detail = detail.is_some();
        let rows_n = d.cols.clamp(1, SPEC_ROWS);
        let mut u = [0.0f32; U_FLOATS];
        let put = |u: &mut [f32; U_FLOATS], i: usize, v: [f32; 4]| {
            u[i * 4..i * 4 + 4].copy_from_slice(&v)
        };
        put(
            &mut u,
            0,
            [
                kind as u8 as f32,
                d.cols as f32,
                d.lanes as f32,
                d.steps as f32,
            ],
        );
        put(
            &mut u,
            1,
            [
                look.start as f32,
                look.span as f32,
                look.played_to.map_or(-1.0, |p| p as f32),
                rows_n as f32,
            ],
        );
        put(
            &mut u,
            2,
            [
                px[2] as f32,
                px[3] as f32,
                (d.duration_s * 10.0) as f32,
                lane_gap,
            ],
        );
        put(&mut u, 3, look.bg);
        put(&mut u, 4, look.axis);
        put(&mut u, 5, look.wave);
        put(&mut u, 6, look.rms);
        put(&mut u, 7, look.played);
        put(&mut u, 8, look.played_rms);
        put(
            &mut u,
            9,
            [-60.0, 0.0, d.integrated.map_or(-200.0, |i| i as f32), 0.0],
        );
        put(&mut u, 10, look.mark);
        self.panels.push(Panel {
            px,
            u,
            detail: is_detail,
        });
    }

    /// Draws the queued panels into the target.
    pub(super) fn render(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder) {
        let panels = std::mem::take(&mut self.panels);
        let (Some(d), Some((_, view, size))) = (&self.data, &self.target) else {
            return;
        };
        if panels.is_empty() {
            return;
        }
        for (p, buf) in panels.iter().zip(&self.uniforms) {
            let bytes: Vec<u8> = p.u.iter().flat_map(|x| x.to_le_bytes()).collect();
            gpu.queue.write_buffer(buf, 0, &bytes);
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sound page"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        for (i, p) in panels.iter().enumerate() {
            let [x, y, w, h] = p.px;
            let (x, y) = (x.min(size.0 - 1), y.min(size.1 - 1));
            let (w, h) = (w.min(size.0 - x), h.min(size.1 - y));
            if w == 0 || h == 0 {
                continue;
            }
            pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
            pass.set_scissor_rect(x, y, w, h);
            let groups = match (&self.detail, p.detail) {
                (Some((dd, _, _)), true) => &dd.groups,
                _ => &d.groups,
            };
            pass.set_bind_group(0, &groups[i], &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
