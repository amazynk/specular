//! Overlays on planes: segment labels ([`LabelRenderer`]: SEG, RTSTRUCT and TMTV masks, filled
//! with alpha and outlined) and fusion ([`FusionRenderer`]: a second series through a
//! [`Colormap`], blended over the first), with [`Rigid`] moving the second series against the
//! first. Both draw into the HDR scene after the planes underneath, premultiplied.
//! [`labels_reference`] and [`fusion_reference`] are the same on the CPU, for tests.

use specular_dicom::volume::{Volume, VoxelKind, Voxels};
use specular_gfx::{Gpu, HDR, fullscreen_pipeline, shader, wgpu};

use crate::{Plane, UNIFORM_FLOATS, VolumeTexture, dot, reference, uniforms};

/// Colour maps for fusion overlays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Colormap {
    /// Black → red → yellow → white.
    #[default]
    HotIron,
    /// Blue → cyan → green → yellow → red.
    Rainbow,
    /// Perceptually even purple → green → yellow.
    Viridis,
}

const VIRIDIS: [[u8; 3]; 10] = [
    [0x44, 0x01, 0x54],
    [0x48, 0x28, 0x78],
    [0x3e, 0x49, 0x89],
    [0x31, 0x68, 0x8e],
    [0x26, 0x82, 0x8e],
    [0x1f, 0x9e, 0x89],
    [0x35, 0xb7, 0x79],
    [0x6e, 0xce, 0x58],
    [0xb5, 0xde, 0x2b],
    [0xfd, 0xe7, 0x25],
];

impl Colormap {
    pub const ALL: [Colormap; 3] = [Colormap::HotIron, Colormap::Rainbow, Colormap::Viridis];

    /// The next map in [`Colormap::ALL`], wrapping.
    pub fn next(self) -> Colormap {
        let i = Colormap::ALL.iter().position(|&c| c == self).unwrap_or(0);
        Colormap::ALL[(i + 1) % Colormap::ALL.len()]
    }

    /// Stable name (settings, i18n ids).
    pub fn id(self) -> &'static str {
        match self {
            Colormap::HotIron => "hot-iron",
            Colormap::Rainbow => "rainbow",
            Colormap::Viridis => "viridis",
        }
    }

    /// The sRGB colour of entry `i` (0 the window's bottom, 255 its top).
    pub fn srgb(self, i: u8) -> [u8; 3] {
        let i = u32::from(i);
        match self {
            Colormap::HotIron => [
                (2 * i).min(255) as u8,
                if i < 128 { 0 } else { (2 * (i - 128)) as u8 },
                if i < 192 { 0 } else { (4 * (i - 192)) as u8 },
            ],
            Colormap::Rainbow => {
                let t = i as f64 / 255.0;
                let ch =
                    |c: f64| ((1.5 - (4.0 * t - c).abs()).clamp(0.0, 1.0) * 255.0).round() as u8;
                [ch(3.0), ch(2.0), ch(1.0)]
            }
            Colormap::Viridis => {
                let f = i as f64 / 255.0 * (VIRIDIS.len() - 1) as f64;
                let k = (f.floor() as usize).min(VIRIDIS.len() - 2);
                let t = f - k as f64;
                [0, 1, 2].map(|c| {
                    let (a, b) = (f64::from(VIRIDIS[k][c]), f64::from(VIRIDIS[k + 1][c]));
                    (a + (b - a) * t).round() as u8
                })
            }
        }
    }

    /// The 256 entries in linear light (rgb, 1), as the shader reads them.
    pub fn linear(self) -> Vec<[f32; 4]> {
        (0..=255u8)
            .map(|i| {
                let c = self
                    .srgb(i)
                    .map(|v| srgb_to_linear(f64::from(v) / 255.0) as f32);
                [c[0], c[1], c[2], 1.0]
            })
            .collect()
    }
}

/// sRGB-coded 0..=1 to linear light.
pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// A rigid move of the second (moving) series against the first: rotation about `centre`
/// (degrees about the patient x, then y, then z axis), then translation (mm).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rigid {
    pub translate: [f64; 3],
    pub rotate_deg: [f64; 3],
    pub centre: [f64; 3],
}

impl Rigid {
    pub fn is_identity(&self) -> bool {
        self.translate == [0.0; 3] && self.rotate_deg == [0.0; 3]
    }

    /// The rotation R = Rz · Ry · Rx (rows).
    pub fn matrix(&self) -> [[f64; 3]; 3] {
        let [a, b, c] = self.rotate_deg.map(f64::to_radians);
        let (sa, ca) = a.sin_cos();
        let (sb, cb) = b.sin_cos();
        let (sc, cc) = c.sin_cos();
        [
            [cc * cb, cc * sb * sa - sc * ca, cc * sb * ca + sc * sa],
            [sc * cb, sc * sb * sa + cc * ca, sc * sb * ca - cc * sa],
            [-sb, cb * sa, cb * ca],
        ]
    }

    /// Where moving-series position `p` is shown: R (p − c) + c + t.
    pub fn to_fixed(&self, p: [f64; 3]) -> [f64; 3] {
        let r = self.matrix();
        let d = [0, 1, 2].map(|i| p[i] - self.centre[i]);
        [0, 1, 2].map(|i| dot(r[i], d) + self.centre[i] + self.translate[i])
    }

    /// The moving-series position shown at fixed position `p`: Rᵀ (p − c − t) + c.
    pub fn to_moving(&self, p: [f64; 3]) -> [f64; 3] {
        let d = [0, 1, 2].map(|i| p[i] - self.centre[i] - self.translate[i]);
        let r = self.rotate_t(d);
        [0, 1, 2].map(|i| r[i] + self.centre[i])
    }

    fn rotate_t(&self, d: [f64; 3]) -> [f64; 3] {
        let r = self.matrix();
        [0, 1, 2].map(|i| r[0][i] * d[0] + r[1][i] * d[1] + r[2][i] * d[2])
    }

    /// `plane` of the fixed series as the matching plane through the moving one (same target
    /// pixels): its origin and steps carried by [`Rigid::to_moving`].
    pub fn plane(&self, plane: &Plane) -> Plane {
        Plane {
            origin: self.to_moving(plane.origin),
            du: self.rotate_t(plane.du),
            dv: self.rotate_t(plane.dv),
            ..*plane
        }
    }
}

/// A label volume as the GPU draws it: its texture, for a fractional segment's layer that
/// segment's index, and where its segments' colours start in [`LabelStyle::colours`] (several
/// segmentations share one draw, each from its own `base`).
#[derive(Clone, Copy, Debug)]
pub struct LabelLayer<'a> {
    pub texture: &'a VolumeTexture,
    pub fraction: Option<usize>,
    pub base: usize,
}

/// How segments are drawn: per segment linear rgb and opacity (0 hides it), the fill's
/// share of that opacity, and the outline's width in target px (0: none).
#[derive(Clone, Debug, PartialEq)]
pub struct LabelStyle {
    pub colours: Vec<[f32; 4]>,
    pub fill: f32,
    pub outline: f32,
}

impl LabelStyle {
    /// Linear rgb and opacity from an sRGB colour.
    pub fn colour(srgb: [u8; 3], opacity: f32) -> [f32; 4] {
        let c = srgb.map(|v| srgb_to_linear(f64::from(v) / 255.0) as f32);
        [c[0], c[1], c[2], opacity]
    }
}

#[derive(Debug)]
struct Pipe {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

#[derive(Debug)]
struct Slot {
    buf: wgpu::Buffer,
    /// The bind group, for (volume id, storage generation).
    group: Option<((u64, u64), wgpu::BindGroup)>,
}

/// The pipeline of `entry` in `source` for textures of `kind`: bindings as mpr.wgsl plus a
/// read-only storage buffer at 3.
fn make_pipe(gpu: &Gpu, label: &str, source: &str, entry: &str, kind: VoxelKind) -> Pipe {
    let ty = match kind {
        VoxelKind::I16 => wgpu::TextureSampleType::Sint,
        VoxelKind::U16 => wgpu::TextureSampleType::Uint,
        VoxelKind::F32 => wgpu::TextureSampleType::Float { filterable: false },
    };
    let e = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    };
    let storage = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                e(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                e(
                    1,
                    wgpu::BindingType::Texture {
                        sample_type: ty,
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                ),
                e(2, storage),
                e(3, storage),
            ],
        });
    let module = shader(&gpu.device, label, source);
    let pipeline = fullscreen_pipeline(
        gpu,
        label,
        &module,
        entry,
        &[Some(&layout)],
        HDR,
        Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
    );
    Pipe { layout, pipeline }
}

fn sample_type(kind: VoxelKind) -> &'static str {
    match kind {
        VoxelKind::I16 => "i32",
        VoxelKind::U16 => "u32",
        VoxelKind::F32 => "f32",
    }
}

/// The labels shader (mpr.wgsl for R16Uint plus `shaders/labels.wgsl`).
pub fn labels_shader_source() -> String {
    format!(
        "{}\n{}",
        crate::shader_source("u32"),
        include_str!("shaders/labels.wgsl")
    )
}

/// The fusion shader for one texture type (mpr.wgsl plus `shaders/fusion.wgsl`).
pub fn fusion_shader_source(sample: &str) -> String {
    format!(
        "{}\n{}",
        crate::shader_source(sample),
        include_str!("shaders/fusion.wgsl")
    )
}

/// Uniform slots and one storage buffer, shared by both overlay renderers.
#[derive(Debug, Default)]
struct Draws {
    slots: Vec<Slot>,
    storage: Option<wgpu::Buffer>,
    generation: u64,
}

impl Draws {
    /// Writes `data` to the storage buffer (regrown when too small).
    fn storage(&mut self, gpu: &Gpu, data: &[[f32; 4]]) {
        let bytes = (data.len().max(1) * 16) as u64;
        if self.storage.as_ref().is_none_or(|b| b.size() < bytes) {
            self.storage = Some(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("overlay colours"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.generation += 1;
        }
        if !data.is_empty() {
            let buf = self.storage.as_ref().expect("made above");
            gpu.queue.write_buffer(buf, 0, bytemuck::cast_slice(data));
        }
    }

    /// Draws each (volume, uniforms, rectangle) with `pipe`.
    fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        pipe: &Pipe,
        items: &[(&VolumeTexture, [f32; UNIFORM_FLOATS], [u32; 4])],
    ) {
        while self.slots.len() < items.len() {
            let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("overlay plane"),
                size: (UNIFORM_FLOATS * 4) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.slots.push(Slot { buf, group: None });
        }
        let Some(storage) = self.storage.as_ref() else {
            return;
        };
        let mut draws = Vec::with_capacity(items.len());
        for (slot, (vol, u, rect)) in self.slots.iter_mut().zip(items) {
            let x0 = rect[0].min(size.0);
            let y0 = rect[1].min(size.1);
            let x1 = rect[0].saturating_add(rect[2]).min(size.0);
            let y1 = rect[1].saturating_add(rect[3]).min(size.1);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            gpu.queue
                .write_buffer(&slot.buf, 0, bytemuck::cast_slice(u));
            let key = (vol.id, self.generation);
            if slot.group.as_ref().is_none_or(|(k, _)| *k != key) {
                let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("overlay"),
                    layout: &pipe.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: slot.buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(vol.view()),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: vol.lut().as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: storage.as_entire_binding(),
                        },
                    ],
                });
                slot.group = Some((key, group));
            }
            draws.push((
                [x0, y0, x1 - x0, y1 - y0],
                &slot.group.as_ref().expect("made above").1,
            ));
        }
        if draws.is_empty() {
            return;
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay"),
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
        pass.set_pipeline(&pipe.pipeline);
        for (r, group) in draws {
            pass.set_scissor_rect(r[0], r[1], r[2], r[3]);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// Draws label layers over [`Plane`]s. One draw per frame and renderer: its buffers are
/// rewritten by the next (draw several segmentations together, see [`LabelLayer::base`]).
#[derive(Debug, Default)]
pub struct LabelRenderer {
    pipe: Option<Pipe>,
    draws: Draws,
}

impl LabelRenderer {
    pub fn new() -> LabelRenderer {
        LabelRenderer::default()
    }

    /// Draws `layers` (R16Uint label volumes) over `planes` (their thin planes; a slab's
    /// labels are those of its centre plane) in `style`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        layers: &[LabelLayer],
        style: &LabelStyle,
        planes: &[Plane],
    ) {
        let layers: Vec<&LabelLayer> = layers
            .iter()
            .filter(|l| l.texture.kind() == VoxelKind::U16)
            .collect();
        if layers.is_empty() || planes.is_empty() || style.colours.is_empty() {
            return;
        }
        let pipe = self.pipe.get_or_insert_with(|| {
            make_pipe(
                gpu,
                "labels",
                &labels_shader_source(),
                "fs_labels",
                VoxelKind::U16,
            )
        });
        self.draws.storage(gpu, &style.colours);
        let mut items = Vec::with_capacity(layers.len() * planes.len());
        for plane in planes {
            let thin = Plane {
                thickness: 0.0,
                ..*plane
            };
            for l in &layers {
                if let Some(s) = l.fraction
                    && style.colours.get(l.base + s).is_none_or(|c| c[3] <= 0.0)
                {
                    continue;
                }
                let mut u = uniforms(l.texture, &thin);
                u[32..36].copy_from_slice(&[
                    l.fraction.map_or(-1.0, |s| s as f32),
                    style.fill,
                    style.outline,
                    l.base as f32,
                ]);
                items.push((l.texture, u, plane.rect));
            }
        }
        self.draws.draw(gpu, enc, target, size, pipe, &items);
    }
}

/// Draws a second series over [`Plane`]s through a colour map (one draw per frame and
/// renderer, as [`LabelRenderer`]).
#[derive(Debug, Default)]
pub struct FusionRenderer {
    pipes: [Option<Pipe>; 3],
    draws: Draws,
    colormap: Option<Colormap>,
}

impl FusionRenderer {
    pub fn new() -> FusionRenderer {
        FusionRenderer::default()
    }

    /// Draws `vol` over `planes` (planes of the moving series: [`Rigid::plane`] of the fixed
    /// ones, with the moving series' window and slab) through `colormap` at opacity `blend`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        vol: &VolumeTexture,
        colormap: Colormap,
        blend: f32,
        planes: &[Plane],
    ) {
        if planes.is_empty() || blend <= 0.0 {
            return;
        }
        let kind = vol.kind();
        let v = crate::variant(kind);
        let pipe = self.pipes[v].get_or_insert_with(|| {
            make_pipe(
                gpu,
                "fusion",
                &fusion_shader_source(sample_type(kind)),
                "fs_fusion",
                kind,
            )
        });
        if self.colormap != Some(colormap) || self.draws.storage.is_none() {
            self.draws.storage(gpu, &colormap.linear());
            self.colormap = Some(colormap);
        }
        let items: Vec<_> = planes
            .iter()
            .map(|p| {
                let mut u = uniforms(vol, p);
                u[32..36].copy_from_slice(&[blend.min(1.0), 0.0, 0.0, 0.0]);
                (vol, u, p.rect)
            })
            .collect();
        self.draws.draw(gpu, enc, target, size, pipe, &items);
    }
}

/// The label value nearest `p` as the shader finds it (−1 outside the grid).
fn label_at(vol: &Volume, p: [f64; 3]) -> i32 {
    let Voxels::U16(data) = &vol.voxels else {
        return -1;
    };
    let g = &vol.geometry;
    let a = dot([0, 1, 2].map(|c| p[c] - g.origin[c]), g.normal);
    let n = g.along.len();
    let (lo, hi) = if n < 2 {
        (-0.5, 0.5)
    } else {
        (
            -0.5 * (g.along[1] - g.along[0]),
            g.along[n - 1] + 0.5 * (g.along[n - 1] - g.along[n - 2]),
        )
    };
    if a < lo || a > hi {
        return -1;
    }
    let v = g.to_voxel(p);
    let [w, h, d] = g.dims.map(f64::from);
    let (i, j, k) = (
        (v[0] + 0.5).floor(),
        (v[1] + 0.5).floor(),
        (v[2] + 0.5).floor(),
    );
    if i < 0.0 || j < 0.0 || k < 0.0 || i >= w || j >= h || k >= d {
        return -1;
    }
    let idx = (k as usize * h as usize + j as usize) * w as usize + i as usize;
    i32::from(data[idx])
}

/// (segment index or −1, fraction) at `p`, as the shader's `segment`.
fn segment_at(vol: &Volume, fraction: Option<usize>, p: [f64; 3]) -> (i64, f32) {
    let v = label_at(vol, p);
    if v <= 0 {
        return (-1, 0.0);
    }
    match fraction {
        Some(s) if v >= 32768 => (s as i64, v as f32 / 65535.0),
        Some(_) => (-1, 0.0),
        None => (i64::from(v) - 1, 1.0),
    }
}

/// What [`LabelRenderer`] draws at target pixel centre (`x`, `y`) for one layer, on the CPU:
/// premultiplied linear rgb and alpha, `None` where it draws nothing.
pub fn labels_reference(
    layer: &Volume,
    fraction: Option<usize>,
    base: usize,
    style: &LabelStyle,
    plane: &Plane,
    x: f64,
    y: f64,
) -> Option<[f32; 4]> {
    let p = plane.at(x, y);
    let (s, f) = segment_at(layer, fraction, p);
    if s < 0 {
        return None;
    }
    let c = *style.colours.get(base + s as usize)?;
    if c[3] <= 0.0 {
        return None;
    }
    let w = f64::from(style.outline);
    let edge = w > 0.0 && {
        let a = plane.du.map(|v| v * w);
        let b = plane.dv.map(|v| v * w);
        [a, a.map(|v| -v), b, b.map(|v| -v)]
            .iter()
            .any(|d| segment_at(layer, fraction, [0, 1, 2].map(|i| p[i] + d[i])).0 != s)
    };
    let alpha = if edge { c[3] } else { c[3] * style.fill * f };
    Some([c[0] * alpha, c[1] * alpha, c[2] * alpha, alpha])
}

/// What [`FusionRenderer`] draws at target pixel centre (`x`, `y`) of `plane` (a plane of
/// the moving series), on the CPU: premultiplied linear rgb and alpha, `None` where it draws
/// nothing.
pub fn fusion_reference(
    vol: &Volume,
    colormap: Colormap,
    blend: f32,
    plane: &Plane,
    x: f64,
    y: f64,
) -> Option<[f32; 4]> {
    let v = f64::from(reference(vol, plane, x, y)?);
    let g = plane.window.apply(v);
    if g <= 0.0 {
        return None;
    }
    let g = if plane.window.invert { 1.0 - g } else { g };
    let i = ((g * 255.0 + 0.5) as u32).min(255) as u8;
    let c = colormap
        .srgb(i)
        .map(|c| srgb_to_linear(f64::from(c) / 255.0) as f32);
    let a = blend.min(1.0);
    Some([c[0] * a, c[1] * a, c[2] * a, a])
}

/// One isodose level: its dose (in the volume's units) and sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsodoseLevel {
    pub dose: f64,
    pub srgb: [u8; 3],
}

/// How isodose lines are drawn: the levels (any order), the line width in target px and the
/// opacity of the colour wash between them (0: lines only).
#[derive(Clone, Debug, PartialEq)]
pub struct IsodoseStyle {
    pub levels: Vec<IsodoseLevel>,
    pub width: f32,
    pub wash: f32,
}

impl IsodoseStyle {
    /// The levels lowest first, as the shader reads them: linear rgb, dose.
    fn table(&self) -> Vec<[f32; 4]> {
        let mut l = self.levels.clone();
        l.sort_by(|a, b| a.dose.total_cmp(&b.dose));
        l.iter()
            .map(|v| {
                let c = LabelStyle::colour(v.srgb, 1.0);
                [c[0], c[1], c[2], v.dose as f32]
            })
            .collect()
    }
}

/// The isodose shader (mpr.wgsl for f32 plus `shaders/isodose.wgsl`).
pub fn isodose_shader_source() -> String {
    format!(
        "{}\n{}",
        crate::shader_source("f32"),
        include_str!("shaders/isodose.wgsl")
    )
}

/// Draws isodose lines of a dose volume (f32) over [`Plane`]s (one draw per frame and
/// renderer, as [`LabelRenderer`]).
#[derive(Debug, Default)]
pub struct IsodoseRenderer {
    pipe: Option<Pipe>,
    draws: Draws,
}

impl IsodoseRenderer {
    pub fn new() -> IsodoseRenderer {
        IsodoseRenderer::default()
    }

    /// Draws the isodose lines of `dose` over `planes` (planes of the dose grid; thin, a
    /// slab's lines are those of its centre plane) in `style`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        dose: &VolumeTexture,
        style: &IsodoseStyle,
        planes: &[Plane],
    ) {
        if planes.is_empty() || style.levels.is_empty() || dose.kind() != VoxelKind::F32 {
            return;
        }
        let pipe = self.pipe.get_or_insert_with(|| {
            make_pipe(
                gpu,
                "isodose",
                &isodose_shader_source(),
                "fs_isodose",
                VoxelKind::F32,
            )
        });
        let table = style.table();
        self.draws.storage(gpu, &table);
        let items: Vec<_> = planes
            .iter()
            .map(|p| {
                let thin = Plane {
                    thickness: 0.0,
                    ..*p
                };
                let mut u = uniforms(dose, &thin);
                u[32..36].copy_from_slice(&[table.len() as f32, style.width, style.wash, 0.0]);
                (dose, u, p.rect)
            })
            .collect();
        self.draws.draw(gpu, enc, target, size, pipe, &items);
    }
}

/// How many of `table`'s levels (lowest first) the dose at `p` reaches, as the shader.
fn isodose_band(dose: &Volume, table: &[[f32; 4]], p: [f64; 3]) -> usize {
    let Some(v) = dose.sample_patient(p) else {
        return 0;
    };
    table.iter().rposition(|l| v >= l[3]).map_or(0, |i| i + 1)
}

/// What [`IsodoseRenderer`] draws at target pixel centre (`x`, `y`), on the CPU:
/// premultiplied linear rgb and alpha, `None` where it draws nothing.
pub fn isodose_reference(
    dose: &Volume,
    style: &IsodoseStyle,
    plane: &Plane,
    x: f64,
    y: f64,
) -> Option<[f32; 4]> {
    let table = style.table();
    let p = plane.at(x, y);
    let b = isodose_band(dose, &table, p);
    if b == 0 {
        return None;
    }
    let w = f64::from(style.width);
    let edge = w > 0.0 && {
        let a = plane.du.map(|v| v * w);
        let c = plane.dv.map(|v| v * w);
        [a, a.map(|v| -v), c, c.map(|v| -v)]
            .iter()
            .any(|d| isodose_band(dose, &table, [0, 1, 2].map(|i| p[i] + d[i])) < b)
    };
    let alpha = if edge { 1.0 } else { style.wash };
    if alpha <= 0.0 {
        return None;
    }
    let c = table[b - 1];
    Some([c[0] * alpha, c[1] * alpha, c[2] * alpha, alpha])
}
