//! Volumes on the GPU.
//!
//! [`VolumeTexture`] holds a [`Volume`]'s modality values in one 3D texture (R16Sint, R16Uint
//! or R32Float, as the volume stores them) with the slice lookup the shaders need for uneven
//! spacing, gantry tilt and slices still loading. It uploads slices in runs as they arrive and
//! is the handle later volume renderers share.
//!
//! [`MprRenderer`] draws multiplanar reformats into the HDR scene: any plane (axial, sagittal,
//! coronal, oblique), thin or as a thick slab (MIP, MinIP, average), sampled trilinearly in
//! patient millimetres (so aspect follows the voxel spacing) with the VOI window applied in
//! the shader. [`reference`] is the same sampling on the CPU, for tests and readouts.
//!
//! [`VrRenderer`] ray-marches the same texture for the 3D view: transfer function presets
//! ([`tf`]), shaded or cinematic compositing (shadows and ambient occlusion refined over
//! frames), MIP and MinIP, clipping, early termination and empty-space skipping over
//! [`Bricks`]. [`isosurface`] extracts the surface at a threshold as a mesh in patient
//! millimetres for STL export.
//!
//! [`overlay`] draws segment labels and fused series over those planes.

use std::sync::atomic::{AtomicU64, Ordering};

pub mod arch;
pub mod bricks;
pub mod curve;
pub mod iso;
pub mod mesh_ops;
pub mod tf;
pub mod vr;

pub use bricks::Bricks;
pub use curve::{Cpr, CprKind, CprView, Spline, cross_section};
pub use iso::{Mesh, isosurface, label_surface};
pub use tf::{Preset, Transfer};
pub use vr::{Clip, Mode, View3d, VrRenderer};

use specular_core::nd::Stretch;
use specular_dicom::volume::{Geometry, Volume, VoxelKind, Voxels};
use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, shader, wgpu};
use specular_render::{Gsdf, Window};

pub mod overlay;
pub use overlay::{
    Colormap, FusionRenderer, IsodoseLevel, IsodoseRenderer, IsodoseStyle, LabelLayer,
    LabelRenderer, LabelStyle, Rigid, fusion_reference, isodose_reference, labels_reference,
};

/// How a slab's samples combine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Slab {
    /// Average intensity projection (also the thin plane's mode).
    #[default]
    Average,
    /// Maximum intensity projection.
    Mip,
    /// Minimum intensity projection.
    MinIp,
}

impl Slab {
    fn code(self) -> f32 {
        match self {
            Slab::Average => 0.0,
            Slab::Mip => 1.0,
            Slab::MinIp => 2.0,
        }
    }
}

/// Upper bound on samples through a slab.
pub const MAX_SLAB_SAMPLES: u32 = 256;

/// One reformatted plane, drawn into a rectangle of the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// The rectangle drawn (target px: x, y, width, height); nothing outside it changes.
    pub rect: [u32; 4],
    /// The patient position (mm, LPS) at target pixel coordinate (0, 0), the target's corner.
    pub origin: [f64; 3],
    /// Patient mm per target pixel to the right and down.
    pub du: [f64; 3],
    pub dv: [f64; 3],
    /// The slab's thickness (mm) along the plane's normal (`du × dv`); 0 for a thin plane.
    pub thickness: f64,
    pub slab: Slab,
    pub window: Window,
    /// Linear-light colour outside the volume.
    pub canvas: [f32; 4],
}

impl Plane {
    /// A plane whose rectangle's centre shows patient position `centre`, with unit
    /// directions `right` and `down` and `mm` patient mm per target pixel.
    pub fn centred(
        rect: [u32; 4],
        centre: [f64; 3],
        right: [f64; 3],
        down: [f64; 3],
        mm: f64,
        window: Window,
    ) -> Plane {
        let cx = rect[0] as f64 + rect[2] as f64 / 2.0;
        let cy = rect[1] as f64 + rect[3] as f64 / 2.0;
        let du = right.map(|c| c * mm);
        let dv = down.map(|c| c * mm);
        Plane {
            rect,
            origin: [0, 1, 2].map(|c| centre[c] - du[c] * cx - dv[c] * cy),
            du,
            dv,
            thickness: 0.0,
            slab: Slab::Average,
            window,
            canvas: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// The patient position at target pixel coordinate (`x`, `y`).
    pub fn at(&self, x: f64, y: f64) -> [f64; 3] {
        [0, 1, 2].map(|c| self.origin[c] + self.du[c] * x + self.dv[c] * y)
    }

    /// The unit normal (`du × dv`).
    pub fn normal(&self) -> [f64; 3] {
        let n = cross(self.du, self.dv);
        let l = dot(n, n).sqrt();
        if l > 0.0 {
            n.map(|c| c / l)
        } else {
            [0.0, 0.0, 1.0]
        }
    }

    /// The slab's sample offsets (mm along the normal) for a volume of `geometry`: one at 0
    /// for a thin plane, else evenly over ±thickness/2 no further apart than the smallest
    /// voxel edge (at most [`MAX_SLAB_SAMPLES`]).
    pub fn offsets(&self, geometry: &Geometry) -> (f64, f64, u32) {
        if self.thickness <= 0.0 {
            return (0.0, 0.0, 1);
        }
        let v = geometry.voxel_mm();
        let step = v[0].min(v[1]).min(v[2]).max(1e-3);
        let n = ((self.thickness / step).ceil() as u32 + 1).clamp(2, MAX_SLAB_SAMPLES);
        (-self.thickness / 2.0, self.thickness / (n - 1) as f64, n)
    }
}

pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// What the GPU draws at target pixel centre (`x`, `y`) of `plane`, on the CPU: the slab's
/// samples through [`Volume::sample_patient`], combined; `None` outside the volume.
pub fn reference(vol: &Volume, plane: &Plane, x: f64, y: f64) -> Option<f32> {
    let p = plane.at(x, y);
    let n = plane.normal();
    let (first, step, count) = plane.offsets(&vol.geometry);
    let mut acc: Option<f32> = None;
    let mut hits = 0u32;
    for m in 0..count {
        let o = first + step * m as f64;
        let Some(v) = vol.sample_patient([0, 1, 2].map(|c| p[c] + n[c] * o)) else {
            continue;
        };
        acc = Some(match (acc, plane.slab) {
            (None, _) => v,
            (Some(a), Slab::Mip) => a.max(v),
            (Some(a), Slab::MinIp) => a.min(v),
            (Some(a), Slab::Average) => a + v,
        });
        hits += 1;
    }
    match plane.slab {
        Slab::Average => acc.map(|a| a / hits as f32),
        _ => acc,
    }
}

fn format(kind: VoxelKind) -> wgpu::TextureFormat {
    match kind {
        VoxelKind::I16 => wgpu::TextureFormat::R16Sint,
        VoxelKind::U16 => wgpu::TextureFormat::R16Uint,
        VoxelKind::F32 => wgpu::TextureFormat::R32Float,
    }
}

fn variant(kind: VoxelKind) -> usize {
    match kind {
        VoxelKind::I16 => 0,
        VoxelKind::U16 => 1,
        VoxelKind::F32 => 2,
    }
}

/// The largest volume edge the GPU takes (the app plans its volumes within it, downsampling
/// when a series is larger).
pub fn max_dimension(gpu: &Gpu) -> u32 {
    gpu.limits.max_texture_dimension_3d
}

/// Most bytes one upload copies: slices go up in runs of at most this.
pub const UPLOAD_CHUNK: u64 = 16 << 20;

static IDS: AtomicU64 = AtomicU64::new(1);

/// A volume on the GPU (see the crate docs). Follows its [`Volume`] through
/// [`VolumeTexture::sync`].
#[derive(Debug)]
pub struct VolumeTexture {
    id: u64,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    lut: wgpu::Buffer,
    kind: VoxelKind,
    geometry: Geometry,
    /// Slice table: a of the first entry, mm between entries, entries.
    table: (f64, f64, usize),
    uploaded: Vec<bool>,
}

impl VolumeTexture {
    /// The texture for `vol` (with nothing uploaded yet; see [`VolumeTexture::sync`]).
    /// Refuses a volume over the GPU's 3D texture or storage limits.
    pub fn new(gpu: &Gpu, vol: &Volume) -> Result<VolumeTexture, String> {
        let g = &vol.geometry;
        let max = max_dimension(gpu);
        if g.dims.iter().any(|&d| d == 0 || d > max) {
            return Err(format!(
                "volume {}×{}×{} exceeds the GPU's 3D texture limit of {max}",
                g.dims[0], g.dims[1], g.dims[2]
            ));
        }
        let table = g.table(g.table_nodes());
        let nodes = table.entries.len();
        let lut_bytes = ((nodes + g.slices()) * 16) as u64;
        if lut_bytes > gpu.limits.max_storage_buffer_binding_size {
            return Err("the volume's slice table exceeds the GPU's storage limit".into());
        }
        let kind = vol.voxels.kind();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("volume"),
            size: wgpu::Extent3d {
                width: g.dims[0],
                height: g.dims[1],
                depth_or_array_layers: g.dims[2],
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: format(kind),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let lut = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume slice table"),
            size: lut_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue
            .write_buffer(&lut, 0, bytemuck::cast_slice(&table.entries));
        let t = VolumeTexture {
            id: IDS.fetch_add(1, Ordering::Relaxed),
            texture,
            view,
            lut,
            kind,
            geometry: g.clone(),
            table: (table.start, table.step, nodes),
            uploaded: vec![false; g.slices()],
        };
        t.write_neighbours(gpu);
        Ok(t)
    }

    /// Uploads the slices `vol` loaded since the last call (in runs of at most
    /// [`UPLOAD_CHUNK`]) and the neighbour table; recreates the texture when the volume
    /// changed its value type. True when anything changed.
    pub fn sync(&mut self, gpu: &Gpu, vol: &Volume) -> Result<bool, String> {
        if vol.voxels.kind() != self.kind || vol.geometry.dims != self.geometry.dims {
            *self = VolumeTexture::new(gpu, vol)?;
        }
        let d = self.uploaded.len();
        let per = vol.slice_len() as u64 * self.kind.bytes();
        let most = (UPLOAD_CHUNK / per.max(1)).max(1) as usize;
        let mut changed = false;
        let mut k = 0;
        while k < d {
            if self.uploaded[k] || !vol.loaded[k] {
                k += 1;
                continue;
            }
            let start = k;
            while k < d && k - start < most && !self.uploaded[k] && vol.loaded[k] {
                self.uploaded[k] = true;
                k += 1;
            }
            self.upload(gpu, vol, start, k - start);
            changed = true;
        }
        if changed {
            self.write_neighbours(gpu);
        }
        Ok(changed)
    }

    fn upload(&self, gpu: &Gpu, vol: &Volume, k: usize, n: usize) {
        let len = vol.slice_len();
        let range = k * len..(k + n) * len;
        let bytes: &[u8] = match &vol.voxels {
            Voxels::I16(v) => bytemuck::cast_slice(&v[range]),
            Voxels::U16(v) => bytemuck::cast_slice(&v[range]),
            Voxels::F32(v) => bytemuck::cast_slice(&v[range]),
        };
        let [w, h, _] = self.geometry.dims;
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: k as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * self.kind.bytes() as u32),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: n as u32,
            },
        );
    }

    /// The neighbour table over the uploaded slices (as [`Volume::neighbours`] over the
    /// loaded ones; a slice loaded but not yet uploaded is bridged like a missing one).
    fn write_neighbours(&self, gpu: &Gpu) {
        let rows = neighbours(&self.uploaded);
        gpu.queue.write_buffer(
            &self.lut,
            (self.table.2 * 16) as u64,
            bytemuck::cast_slice(&rows),
        );
    }

    /// The geometry the texture was made for.
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub fn kind(&self) -> VoxelKind {
        self.kind
    }

    /// Slices on the GPU.
    pub fn uploaded(&self) -> usize {
        self.uploaded.iter().filter(|&&u| u).count()
    }

    /// GPU memory held (texture and slice table).
    pub fn bytes(&self) -> u64 {
        let [w, h, d] = self.geometry.dims;
        u64::from(w) * u64::from(h) * u64::from(d) * self.kind.bytes() + self.lut.size()
    }

    /// The texture view (one level, the volume's format) and slice-table buffer, for other
    /// volume renderers.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn lut(&self) -> &wgpu::Buffer {
        &self.lut
    }
}

/// [`Volume::neighbours`] for a loaded mask.
fn neighbours(loaded: &[bool]) -> Vec<[f32; 4]> {
    let n = loaded.len();
    let mut out = vec![[-1.0, n as f32, 0.0, 0.0]; n];
    let mut last = -1.0;
    for (k, o) in out.iter_mut().enumerate() {
        if loaded[k] {
            last = k as f32;
        }
        o[0] = last;
    }
    let mut next = n as f32;
    for k in (0..n).rev() {
        if loaded[k] {
            next = k as f32;
        }
        out[k][1] = next;
    }
    out
}

/// The shader source for one texture type (`f32`, `i32` or `u32`).
pub fn shader_source(sample: &str) -> String {
    with_sampler(include_str!("shaders/mpr.wgsl"), sample)
}

/// A volume shader with the shared sampling functions and the fullscreen vertex stage, for
/// one texture type.
pub(crate) fn with_sampler(src: &str, sample: &str) -> String {
    format!(
        "{FULLSCREEN_WGSL}\n{}\n{}",
        include_str!("shaders/sample.wgsl").replace("VTYPE", sample),
        src.replace("VTYPE", sample)
    )
}

/// Knots of the histogram equalisation curve a plane takes ([`tone_curve`]).
pub const TONE_KNOTS: usize = 256;

/// Where the GSDF row sits (after the curve's knots).
const GSDF_AT: usize = 44 + TONE_KNOTS;
const UNIFORM_FLOATS: usize = GSDF_AT + 4;

/// One volume's planes in [`MprRenderer::draw_layers`]: drawn in `tint` (linear RGB; white
/// is grey), added over the layers before it.
#[derive(Clone, Copy, Debug)]
pub struct Layer<'a> {
    pub vol: &'a VolumeTexture,
    pub planes: &'a [Plane],
    pub tint: [f32; 3],
    /// The 2D view's stretch applied after the window, as the science view applies it
    /// ([`specular_core::nd::apply_mid`]).
    pub stretch: Stretch,
    /// The histogram equalisation curve for [`Stretch::HistEq`] ([`tone_curve`]; empty:
    /// linear).
    pub curve: &'a [f32],
    /// The midtones balance of [`Stretch::Auto`] (the window being the automatic
    /// stretch's limits; 0.5 leaves values as they are).
    pub mid: f32,
}

/// The [`TONE_KNOTS`]-knot curve the shader interpolates for histogram equalisation, from
/// a table of [`specular_core::nd::histeq_lut`] over the window's ends.
pub fn tone_curve(lut: &[f32]) -> Vec<f32> {
    (0..TONE_KNOTS)
        .map(|k| {
            let x = k as f32 / (TONE_KNOTS - 1) as f32;
            specular_core::nd::apply(Stretch::HistEq, x, lut)
        })
        .collect()
}

/// What the shader does to a windowed grey value `x` (0..=1) with `stretch`, `curve`
/// ([`tone_curve`]) and Auto's midtones balance `mid`: the CPU reference it is tested
/// against.
pub fn tone(stretch: Stretch, x: f64, curve: &[f32], mid: f32) -> f64 {
    let x = x.clamp(0.0, 1.0);
    match stretch {
        Stretch::HistEq if curve.len() == TONE_KNOTS => {
            let f = x * (TONE_KNOTS - 1) as f64;
            let i = (f.floor() as usize).min(TONE_KNOTS - 2);
            let t = f - i as f64;
            f64::from(curve[i]) * (1.0 - t) + f64::from(curve[i + 1]) * t
        }
        Stretch::Sqrt | Stretch::Log | Stretch::Asinh => {
            f64::from(specular_core::nd::apply(stretch, x as f32, &[]))
        }
        Stretch::Auto => f64::from(specular_core::nd::mtf(mid, x as f32)),
        _ => x,
    }
}

/// The stretch's code in the plane's uniform block (0: none).
fn tone_code(stretch: Stretch, curve: &[f32]) -> f32 {
    match stretch {
        Stretch::Sqrt => 1.0,
        Stretch::Log => 2.0,
        Stretch::Asinh => 3.0,
        Stretch::HistEq if curve.len() == TONE_KNOTS => 4.0,
        Stretch::Auto => 5.0,
        _ => 0.0,
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
    group: Option<(u64, wgpu::BindGroup)>,
}

/// Draws [`Plane`]s of a [`VolumeTexture`] into the HDR scene; several volumes' planes
/// (the channels of a microscopy stack) added in their colours.
#[derive(Debug, Default)]
pub struct MprRenderer {
    /// Per voxel kind: the plane drawn over the target, and added to it.
    pipes: [Option<Pipe>; 3],
    add_pipes: [Option<Pipe>; 3],
    slots: Vec<Slot>,
    /// Grey planes shown on the DICOM Grayscale Standard Display Function, as the 2D
    /// view shows them (`None`: sRGB greys).
    gsdf: Option<Gsdf>,
}

impl MprRenderer {
    pub fn new() -> MprRenderer {
        MprRenderer::default()
    }

    /// Shows grey planes on a GSDF display between its black and white (`None`: sRGB
    /// greys), so a window looks the same as in the 2D view. Colour composites (tinted
    /// layers) stay as they are.
    pub fn set_gsdf(&mut self, gsdf: Option<Gsdf>) {
        self.gsdf = gsdf.filter(|g| g.l_max > g.l_min && g.j_max > g.j_min);
    }

    fn pipe(&mut self, gpu: &Gpu, kind: VoxelKind, additive: bool) -> &Pipe {
        let v = variant(kind);
        let slot = if additive {
            &mut self.add_pipes[v]
        } else {
            &mut self.pipes[v]
        };
        slot.get_or_insert_with(|| {
            let (ty, sample) = match kind {
                VoxelKind::I16 => (wgpu::TextureSampleType::Sint, "i32"),
                VoxelKind::U16 => (wgpu::TextureSampleType::Uint, "u32"),
                VoxelKind::F32 => (wgpu::TextureSampleType::Float { filterable: false }, "f32"),
            };
            let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty,
                count: None,
            };
            let layout = gpu
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("mpr"),
                    entries: &[
                        entry(
                            0,
                            wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                        ),
                        entry(
                            1,
                            wgpu::BindingType::Texture {
                                sample_type: ty,
                                view_dimension: wgpu::TextureViewDimension::D3,
                                multisampled: false,
                            },
                        ),
                        entry(
                            2,
                            wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                        ),
                    ],
                });
            let module = shader(&gpu.device, "mpr", &shader_source(sample));
            // Added: colour summed, the target's alpha kept.
            let blend = additive.then_some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            });
            let pipeline =
                fullscreen_pipeline(gpu, "mpr", &module, "fs_mpr", &[Some(&layout)], HDR, blend);
            Pipe { layout, pipeline }
        })
    }

    /// Draws `planes` of `vol` into `target` (the HDR scene, `size` px), each inside its
    /// rectangle; the rest of the target is kept.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        vol: &VolumeTexture,
        planes: &[Plane],
    ) {
        self.draw_layers(
            gpu,
            enc,
            target,
            size,
            &[Layer {
                vol,
                planes,
                tint: [1.0; 3],
                stretch: Stretch::Linear,
                curve: &[],
                mid: 0.5,
            }],
        );
    }

    /// Draws the first layer's planes as [`MprRenderer::draw`] does, in its tint, then adds
    /// each further layer's in its own (outside its volume it adds nothing): a colour
    /// composite of channels, each through its plane's window.
    pub fn draw_layers(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        layers: &[Layer<'_>],
    ) {
        for (k, l) in layers.iter().enumerate() {
            self.pipe(gpu, l.vol.kind, k > 0);
        }
        let total: usize = layers.iter().map(|l| l.planes.len()).sum();
        while self.slots.len() < total {
            let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mpr plane"),
                size: (UNIFORM_FLOATS * 4) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.slots.push(Slot { buf, group: None });
        }
        let mut draws = Vec::with_capacity(total);
        let jobs = layers
            .iter()
            .enumerate()
            .flat_map(|(k, l)| l.planes.iter().map(move |p| (k, l, p)));
        for (slot, (k, layer, plane)) in self.slots.iter_mut().zip(jobs) {
            let vol = layer.vol;
            let pipe = if k > 0 {
                &self.add_pipes[variant(vol.kind)]
            } else {
                &self.pipes[variant(vol.kind)]
            }
            .as_ref()
            .expect("pipeline made above");
            let x0 = plane.rect[0].min(size.0);
            let y0 = plane.rect[1].min(size.1);
            let x1 = (plane.rect[0].saturating_add(plane.rect[2])).min(size.0);
            let y1 = (plane.rect[1].saturating_add(plane.rect[3])).min(size.1);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let mut u = uniforms(vol, plane);
            u[36..40].copy_from_slice(&[
                layer.tint[0],
                layer.tint[1],
                layer.tint[2],
                if k > 0 { 1.0 } else { 0.0 },
            ]);
            u[40..44].copy_from_slice(&[
                tone_code(layer.stretch, layer.curve),
                specular_core::nd::LOG_A,
                specular_core::nd::ASINH_A,
                layer.mid,
            ]);
            if layer.curve.len() == TONE_KNOTS {
                u[44..GSDF_AT].copy_from_slice(layer.curve);
            }
            let grey = layers.len() == 1 && layer.tint == [1.0; 3];
            if let Some(g) = self.gsdf.filter(|_| grey) {
                u[GSDF_AT..].copy_from_slice(&[
                    g.j_min as f32,
                    g.j_max as f32,
                    g.l_min as f32,
                    (1.0 / (g.l_max - g.l_min)) as f32,
                ]);
            }
            gpu.queue
                .write_buffer(&slot.buf, 0, bytemuck::cast_slice(&u));
            if slot.group.as_ref().is_none_or(|(id, _)| *id != vol.id) {
                let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("mpr"),
                    layout: &pipe.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: slot.buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&vol.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: vol.lut.as_entire_binding(),
                        },
                    ],
                });
                slot.group = Some((vol.id, group));
            }
            draws.push((
                [x0, y0, x1 - x0, y1 - y0],
                &slot.group.as_ref().expect("made above").1,
                &pipe.pipeline,
            ));
        }
        if draws.is_empty() {
            return;
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mpr"),
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
        for (r, group, pipeline) in draws {
            pass.set_pipeline(pipeline);
            pass.set_scissor_rect(r[0], r[1], r[2], r[3]);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// The plane's uniform block (see `shaders/mpr.wgsl`): the plane moved into raw slice space
/// (i and j before the slice's shift, a = mm along the slice normal from slice 0).
fn uniforms(vol: &VolumeTexture, plane: &Plane) -> [f32; UNIFORM_FLOATS] {
    let g = &vol.geometry;
    let raw = |d: [f64; 3]| {
        [
            dot(d, g.row) / g.pixel[0],
            dot(d, g.col) / g.pixel[1],
            dot(d, g.normal),
        ]
    };
    let n = plane.normal();
    let (first, step, count) = plane.offsets(g);
    let start = [0, 1, 2].map(|c| plane.origin[c] - g.origin[c] + n[c] * first);
    let base = raw(start);
    let du = raw(plane.du);
    let dv = raw(plane.dv);
    let dw = raw(n.map(|c| c * step));
    let (voi, mode) = plane.window.rows();
    let (start, step, nodes) = vol.table;
    let mut u = [0f32; UNIFORM_FLOATS];
    let rows: [[f32; 4]; 9] = [
        [base[0] as f32, base[1] as f32, base[2] as f32, 0.0],
        [du[0] as f32, du[1] as f32, du[2] as f32, 0.0],
        [dv[0] as f32, dv[1] as f32, dv[2] as f32, 0.0],
        [dw[0] as f32, dw[1] as f32, dw[2] as f32, count as f32],
        [g.dims[0] as f32, g.dims[1] as f32, g.dims[2] as f32, 0.0],
        [
            start as f32,
            (1.0 / step) as f32,
            nodes as f32,
            g.slices() as f32,
        ],
        voi,
        [mode[0], mode[1], plane.slab.code(), 0.0],
        plane.canvas,
    ];
    for (r, row) in rows.iter().enumerate() {
        u[r * 4..r * 4 + 4].copy_from_slice(row);
    }
    u
}

#[cfg(test)]
mod tests;
