//! Volume rendering of a [`VolumeTexture`]: orthographic rays marched in raw slice space (so
//! uneven spacing and gantry tilt come out exact, through the same slice lookup MPR uses),
//! composited front to back through a [`Transfer`] function with gradient shading, or
//! projected as MIP / MinIP through the VOI window.
//!
//! Rays stop once nearly opaque and jump over bricks the transfer function shows nothing in
//! ([`Bricks`]). Frames accumulate: each jitters the ray start and is blended into a
//! running average, so a few frames remove the sampling pattern and, in
//! [`Mode::Cinematic`], average the random shadow and occlusion rays. While the user drags,
//! [`View3d::interactive`] renders at half resolution with coarser steps.

use specular_dicom::volume::{Geometry, Volume, VoxelKind};
use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, shader, wgpu};
use specular_render::Window;

use crate::bricks::{BRICK, Bricks};
use crate::tf::{LUT_SIZE, Preset, Transfer, lookup};
use crate::{VolumeTexture, cross, dot, variant, with_sampler};

/// How rays combine their samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Composited through the transfer function, lit by a headlight.
    #[default]
    Shaded,
    /// As shaded, with shadows and ambient occlusion refined over frames.
    Cinematic,
    /// Maximum intensity projection through the window.
    Mip,
    /// Minimum intensity projection through the window.
    MinIp,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Shaded, Mode::Cinematic, Mode::Mip, Mode::MinIp];

    /// A stable name (also the Fluent selector).
    pub fn id(self) -> &'static str {
        match self {
            Mode::Shaded => "shaded",
            Mode::Cinematic => "cinematic",
            Mode::Mip => "mip",
            Mode::MinIp => "minip",
        }
    }

    pub fn next(self) -> Mode {
        let i = Mode::ALL.iter().position(|&m| m == self).unwrap_or(0);
        Mode::ALL[(i + 1) % Mode::ALL.len()]
    }

    /// Whether the transfer function applies (else the VOI window does).
    pub fn composite(self) -> bool {
        matches!(self, Mode::Shaded | Mode::Cinematic)
    }

    fn code(self) -> f32 {
        match self {
            Mode::Shaded => 0.0,
            Mode::Cinematic => 1.0,
            Mode::Mip => 2.0,
            Mode::MinIp => 3.0,
        }
    }
}

/// Most planes a clip region keeps to.
pub const MAX_KEEP: usize = 12;

/// Clipping, as planes in patient space (`n·p + d`, `n` a unit normal).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clip {
    /// The region kept: where every one of the first `keeps` planes is ≥ 0.
    pub keep: [[f64; 4]; MAX_KEEP],
    pub keeps: usize,
    /// A corner cut away: where all three planes are ≥ 0.
    pub cut: Option<[[f64; 4]; 3]>,
}

/// One volume rendering, drawn into a rectangle of the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View3d {
    /// The rectangle drawn (target px: x, y, width, height).
    pub rect: [u32; 4],
    /// The patient position (mm, LPS) at the rectangle's centre, on the plane through which
    /// rays enter (any point along the view works: rays cover the whole volume).
    pub centre: [f64; 3],
    /// Unit directions of the rectangle's right and down; the view looks along
    /// `right × down`.
    pub right: [f64; 3],
    pub down: [f64; 3],
    /// Patient mm per target pixel.
    pub mm: f64,
    pub mode: Mode,
    pub preset: Preset,
    /// The series' default window (centre, width), which [`Preset::Mr`] spans.
    pub series: (f64, f64),
    /// The user's window over the transfer function (centre, width): the preset's nominal
    /// window ([`Transfer::window`]) shows it as designed.
    pub tf_window: (f64, f64),
    /// The VOI window of MIP and MinIP.
    pub window: Window,
    pub clip: Clip,
    /// Linear-light colour where rays find nothing.
    pub canvas: [f32; 4],
    /// The user is dragging: half resolution, coarser steps, one frame, no cinematic rays.
    pub interactive: bool,
}

impl View3d {
    /// The direction the view looks along.
    pub fn forward(&self) -> [f64; 3] {
        let f = cross(self.right, self.down);
        let l = dot(f, f).sqrt();
        if l > 0.0 {
            f.map(|c| c / l)
        } else {
            [0.0, 1.0, 0.0]
        }
    }

    pub fn transfer(&self) -> Transfer {
        self.preset.transfer(self.series)
    }

    /// The mode drawn now (cinematic shows as shaded while interactive).
    fn effective(&self) -> Mode {
        if self.interactive && self.mode == Mode::Cinematic {
            Mode::Shaded
        } else {
            self.mode
        }
    }

    /// Frames accumulated before the image is final.
    pub fn frames(&self) -> u32 {
        match self.effective() {
            _ if self.interactive => 1,
            Mode::Shaded => 4,
            Mode::Cinematic => 48,
            Mode::Mip | Mode::MinIp => 1,
        }
    }

    /// The accumulation's size: the rectangle's, halved while interactive.
    pub fn accum_size(&self) -> (u32, u32) {
        let q = if self.interactive { 0.5 } else { 1.0 };
        (
            ((self.rect[2] as f64 * q).ceil() as u32).max(1),
            ((self.rect[3] as f64 * q).ceil() as u32).max(1),
        )
    }
}

/// The rays of a view in raw slice space (shared by the uniforms and [`reference`]).
#[derive(Clone, Debug)]
struct Setup {
    base: [f64; 3],
    len: f64,
    du: [f64; 3],
    dv: [f64; 3],
    dir: [f64; 3],
    step: f64,
    most: u32,
    lo: [f64; 3],
    hi: [f64; 3],
    jump: u32,
    h: f64,
    right: [f64; 3],
    down: [f64; 3],
    light: [f64; 3],
    lcam: [f64; 3],
    shadow: f64,
    mapping: (f64, f64),
    planes: [[f64; 4]; 15],
    keeps: usize,
    cut: bool,
    gradient: (f64, f64),
}

const AMBIENT: f64 = 0.3;

fn setup(g: &Geometry, table: (f64, f64, usize), view: &View3d) -> Setup {
    let raw = |d: [f64; 3]| {
        [
            dot(d, g.row) / g.pixel[0],
            dot(d, g.col) / g.pixel[1],
            dot(d, g.normal),
        ]
    };
    let plane = |p: [f64; 4]| {
        let n = [p[0], p[1], p[2]];
        [
            g.pixel[0] * dot(n, g.row),
            g.pixel[1] * dot(n, g.col),
            dot(n, g.normal),
            dot(n, g.origin) + p[3],
        ]
    };
    let f = view.forward();
    let (aw, ah) = view.accum_size();
    let [rx, ry, rw, rh] = view.rect.map(f64::from);
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let vc = g.centre();
    let half = g
        .corners()
        .iter()
        .map(|c| dot(sub(*c, vc), sub(*c, vc)).sqrt())
        .fold(0.0, f64::max);
    let r = dot(sub(view.centre, vc), sub(view.centre, vc)).sqrt() + half + 1.0;
    let corner = [0, 1, 2].map(|c| {
        view.centre[c] + view.right[c] * view.mm * (rx - cx) + view.down[c] * view.mm * (ry - cy)
            - f[c] * r
    });
    let base = raw(sub(corner, g.origin));
    let du = raw(view.right.map(|c| c * view.mm * rw / aw as f64));
    let dv = raw(view.down.map(|c| c * view.mm * rh / ah as f64));
    let dir = raw(f);
    let v = g.voxel_mm();
    let vmin = v[0].min(v[1]).min(v[2]).max(1e-3);
    let step = vmin * if view.interactive { 1.0 } else { 0.5 };
    let len = 2.0 * r;
    let most = ((len / step).ceil() as u32 + 2).min(1 << 16);
    // The raw box: i and j over every slice's shifted extent, a over the slice table.
    let (mut si, mut sj) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
    for s in &g.shift {
        si = (si.0.min(s[0]), si.1.max(s[0]));
        sj = (sj.0.min(s[1]), sj.1.max(s[1]));
    }
    if g.shift.is_empty() {
        (si, sj) = ((0.0, 0.0), (0.0, 0.0));
    }
    let (start, tstep, nodes) = table;
    let lo = [-0.5 + si.0, -0.5 + sj.0, start];
    let hi = [
        g.dims[0] as f64 - 0.5 + si.1,
        g.dims[1] as f64 - 0.5 + sj.1,
        start + tstep * (nodes.max(2) - 1) as f64,
    ];
    // How fast a ray crosses voxel indices (per mm): bounds the jump over an empty brick.
    let (mut slope_i, mut slope_j, mut inv) = (0f64, 0f64, 0f64);
    for k in 1..g.along.len() {
        let da = (g.along[k] - g.along[k - 1]).max(1e-6);
        slope_i = slope_i.max((g.shift[k][0] - g.shift[k - 1][0]).abs() / da);
        slope_j = slope_j.max((g.shift[k][1] - g.shift[k - 1][1]).abs() / da);
        inv = inv.max(1.0 / da);
    }
    if g.along.len() < 2 {
        inv = 1.0;
    }
    let speed = (dir[0].abs() + slope_i * dir[2].abs())
        .max(dir[1].abs() + slope_j * dir[2].abs())
        .max(inv * dir[2].abs())
        .max(1e-9);
    let jump = ((0.95 * BRICK as f64 / speed / step).floor() as u32).max(1);
    let lcam = normalise([-0.4, -0.5, -1.0]);
    let world =
        |c: [f64; 3]| [0, 1, 2].map(|i| view.right[i] * c[0] + view.down[i] * c[1] + f[i] * c[2]);
    let transfer = view.transfer();
    let mut planes = [[0.0; 4]; 15];
    let keeps = view.clip.keeps.min(MAX_KEEP);
    for (o, p) in planes.iter_mut().zip(&view.clip.keep[..keeps]) {
        *o = plane(*p);
    }
    if let Some(cut) = view.clip.cut {
        for (o, p) in planes[12..].iter_mut().zip(cut) {
            *o = plane(p);
        }
    }
    let w = view.tf_window.1.abs().max(1e-6);
    Setup {
        base,
        len,
        du,
        dv,
        dir,
        step,
        most,
        lo,
        hi,
        jump,
        h: vmin,
        right: raw(view.right),
        down: raw(view.down),
        light: raw(world(lcam)),
        lcam,
        shadow: (2.0 * vmin).max(r / 48.0),
        mapping: transfer.mapping(view.tf_window),
        planes,
        keeps,
        cut: view.clip.cut.is_some(),
        gradient: (0.002 * w, 0.03 * w),
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn normalise(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    a.map(|c| c / l)
}

const ROWS: usize = 16 + 15;

fn uniforms(
    s: &Setup,
    vol: &VolumeTexture,
    view: &View3d,
    bricks: Option<[u32; 3]>,
    frame: u32,
) -> [[f32; 4]; ROWS] {
    let g = &vol.geometry;
    let v3 = |a: [f64; 3], w: f64| [a[0] as f32, a[1] as f32, a[2] as f32, w as f32];
    let (voi, vmode) = view.window.rows();
    let (start, step, nodes) = vol.table;
    let grid = bricks.unwrap_or([1, 1, 1]);
    let mut u = [[0f32; 4]; ROWS];
    let head: [[f32; 4]; 16] = [
        v3(s.base, s.len),
        v3(s.du, s.step),
        v3(s.dv, frame as f64),
        v3(s.dir, s.most as f64),
        [
            g.dims[0] as f32,
            g.dims[1] as f32,
            g.dims[2] as f32,
            if bricks.is_some() { BRICK as f32 } else { 0.0 },
        ],
        [
            start as f32,
            (1.0 / step) as f32,
            nodes as f32,
            g.slices() as f32,
        ],
        v3(s.lo, s.jump as f64),
        v3(s.hi, s.h),
        v3(s.right, f64::from(view.effective().code())),
        v3(s.down, s.keeps as f64),
        v3(s.light, if s.cut { 1.0 } else { 0.0 }),
        [
            s.mapping.0 as f32,
            s.mapping.1 as f32,
            LUT_SIZE as f32,
            AMBIENT as f32,
        ],
        voi,
        [vmode[0], vmode[1], s.gradient.0 as f32, s.gradient.1 as f32],
        [grid[0] as f32, grid[1] as f32, grid[2] as f32, 0.0],
        v3(s.lcam, s.shadow),
    ];
    u[..16].copy_from_slice(&head);
    for (o, p) in u[16..].iter_mut().zip(&s.planes) {
        *o = p.map(|c| c as f32);
    }
    u
}

/// What the GPU accumulates (premultiplied linear RGB and alpha) at accumulation pixel
/// coordinate (`x`, `y`) of `view`'s first frame, on the CPU: the same rays, sampled through
/// [`Volume::sample`]. [`Mode::Cinematic`] is not reproduced (its rays are random); it
/// returns the shaded result.
pub fn reference(vol: &Volume, view: &View3d, x: f64, y: f64) -> [f32; 4] {
    let g = &vol.geometry;
    let table = g.table(g.table_nodes());
    let s = setup(g, (table.start, table.step, table.entries.len()), view);
    let lut = view.transfer().lut();
    let mapping = (s.mapping.0 as f32, s.mapping.1 as f32);
    let top = table.start + table.step * (table.entries.len() - 1) as f64;
    let sample = |q: [f64; 3]| -> Option<f32> {
        if q[2] < table.start || q[2] > top {
            return None;
        }
        let (sl, sh) = g.slice_at(q[2]);
        vol.sample([q[0] - sh[0], q[1] - sh[1], sl])
    };
    let at = |a: [f64; 3], b: [f64; 3], t: f64| [0, 1, 2].map(|c| a[c] + b[c] * t);
    let q0 = [0, 1, 2].map(|c| s.base[c] + s.du[c] * x + s.dv[c] * y);
    let keep = |p: [f64; 4], t: (f64, f64)| {
        let a = p[0] * q0[0] + p[1] * q0[1] + p[2] * q0[2] + p[3];
        let b = p[0] * s.dir[0] + p[1] * s.dir[1] + p[2] * s.dir[2];
        if b.abs() < 1e-12 {
            return if a < 0.0 { (1.0, 0.0) } else { t };
        }
        let r = -a / b;
        if b > 0.0 {
            (t.0.max(r), t.1)
        } else {
            (t.0, t.1.min(r))
        }
    };
    let mut t: (f64, f64) = (0.0, s.len);
    for (c, &q) in q0.iter().enumerate() {
        if s.dir[c].abs() < 1e-12 {
            if q < s.lo[c] || q > s.hi[c] {
                return [0.0; 4];
            }
        } else {
            let a = (s.lo[c] - q) / s.dir[c];
            let b = (s.hi[c] - q) / s.dir[c];
            t = (t.0.max(a.min(b)), t.1.min(a.max(b)));
        }
    }
    for p in &s.planes[..s.keeps] {
        t = keep(*p, t);
    }
    let mut cut: (f64, f64) = (1.0, 0.0);
    if s.cut {
        cut = (-1e30, 1e30);
        for p in &s.planes[12..] {
            cut = keep(*p, cut);
        }
    }
    if t.1 <= t.0 {
        return [0.0; 4];
    }
    let mode = view.effective();
    let mut acc = [0f32; 4];
    let mut ext: Option<f32> = None;
    let mut k = 0u32;
    for _ in 0..s.most {
        let tk = t.0 + (k as f64 + 0.5) * s.step;
        if tk >= t.1 {
            break;
        }
        if tk >= cut.0 && tk < cut.1 {
            k = (k + 1).max(((cut.1 - t.0) / s.step - 0.5).ceil() as u32);
            continue;
        }
        k += 1;
        let q = at(q0, s.dir, tk);
        let Some(v) = sample(q) else { continue };
        match mode {
            Mode::Mip => ext = Some(ext.map_or(v, |e| e.max(v))),
            Mode::MinIp => ext = Some(ext.map_or(v, |e| e.min(v))),
            Mode::Shaded | Mode::Cinematic => {
                let e = lookup(&lut, mapping, v);
                let a = 1.0 - (-e[3] * s.step as f32).exp();
                if a <= 1e-4 {
                    continue;
                }
                let value = |p: [f64; 3]| sample(p).unwrap_or(v);
                let d = |axis: [f64; 3]| {
                    (value(at(q, axis, s.h)) - value(at(q, axis, -s.h))) / (2.0 * s.h as f32)
                };
                let col = shade([d(s.right), d(s.down), d(s.dir)], [e[0], e[1], e[2]], &s);
                let w = 1.0 - acc[3];
                for c in 0..3 {
                    acc[c] += col[c] * a * w;
                }
                acc[3] += a * w;
                if acc[3] > 0.99 {
                    break;
                }
            }
        }
    }
    match (mode, ext) {
        (Mode::Mip | Mode::MinIp, Some(e)) => {
            let l = crate::tf::srgb_to_linear(view.window.grey(f64::from(e)) as f32);
            [l, l, l, 1.0]
        }
        (Mode::Mip | Mode::MinIp, None) => [0.0; 4],
        _ => acc,
    }
}

/// The shader's `shade` without the cinematic rays.
fn shade(g: [f32; 3], base: [f32; 3], s: &Setup) -> [f32; 3] {
    let m = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
    let (e0, e1) = (s.gradient.0 as f32, s.gradient.1 as f32);
    let x = ((m - e0) / (e1 - e0)).clamp(0.0, 1.0);
    let k = x * x * (3.0 - 2.0 * x);
    if k <= 0.0 {
        return base;
    }
    let mut n = g.map(|c| -c / m);
    if n[2] > 0.0 {
        n = n.map(|c| -c);
    }
    let l = s.lcam.map(|c| c as f32);
    let d3 = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let diff = d3(n, l).max(0.0);
    let hv = [l[0], l[1], l[2] - 1.0];
    let hl = d3(hv, hv).sqrt();
    let spec = (d3(n, hv.map(|c| c / hl)).max(0.0)).powf(32.0);
    let amb = AMBIENT as f32;
    [0, 1, 2].map(|c| {
        let shaded = base[c] * (amb + (1.0 - amb) * diff) + 0.25 * spec;
        base[c] + (shaded - base[c]) * k
    })
}

#[derive(Debug)]
struct Pipe {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

#[derive(Debug)]
struct Present {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    buf: wgpu::Buffer,
}

#[derive(Debug)]
struct Accum {
    size: (u32, u32),
    view: wgpu::TextureView,
}

/// A brick table's bricks id, transfer key and window bits.
type BrickKey = (u64, (Preset, [u64; 2]), [u64; 2]);

/// Draws [`View3d`]s of a [`VolumeTexture`] into the HDR scene, refining over frames.
#[derive(Debug, Default)]
pub struct VrRenderer {
    pipes: [Option<Pipe>; 3],
    present: Option<Present>,
    uniform: Option<wgpu::Buffer>,
    /// The transfer table and what it was made for.
    tf: Option<((Preset, [u64; 2]), wgpu::TextureView)>,
    /// The brick table and what it was made for.
    bricks: Option<(BrickKey, wgpu::Buffer)>,
    dummy: Option<wgpu::Buffer>,
    accum: Option<Accum>,
    /// Bumped whenever a bound resource is replaced.
    generation: u64,
    group: Option<((u64, u64), wgpu::BindGroup)>,
    present_group: Option<(u64, wgpu::BindGroup)>,
    key: u64,
    frame: u32,
}

impl VrRenderer {
    pub fn new() -> VrRenderer {
        VrRenderer::default()
    }

    /// Frames accumulated so far for the current view.
    pub fn frame(&self) -> u32 {
        self.frame
    }

    fn pipe(&mut self, gpu: &Gpu, kind: VoxelKind) {
        let v = variant(kind);
        if self.pipes[v].is_some() {
            return;
        }
        let (ty, sample) = match kind {
            VoxelKind::I16 => (wgpu::TextureSampleType::Sint, "i32"),
            VoxelKind::U16 => (wgpu::TextureSampleType::Uint, "u32"),
            VoxelKind::F32 => (wgpu::TextureSampleType::Float { filterable: false }, "f32"),
        };
        let storage = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("vr"),
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
                    entry(2, storage),
                    entry(
                        3,
                        wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D1,
                            multisampled: false,
                        },
                    ),
                    entry(4, storage),
                ],
            });
        let module = shader(&gpu.device, "vr", &shader_source(sample));
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Constant,
            dst_factor: wgpu::BlendFactor::OneMinusConstant,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = fullscreen_pipeline(
            gpu,
            "vr",
            &module,
            "fs_vr",
            &[Some(&layout)],
            HDR,
            Some(wgpu::BlendState {
                color: blend,
                alpha: blend,
            }),
        );
        self.pipes[v] = Some(Pipe { layout, pipeline });
    }

    fn present(&mut self, gpu: &Gpu) -> &Present {
        self.present.get_or_insert_with(|| {
            let layout = gpu
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("vr present"),
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
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                        ),
                        entry(
                            2,
                            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        ),
                    ],
                });
            let module = shader(&gpu.device, "vr present", &present_source());
            let pipeline = fullscreen_pipeline(
                gpu,
                "vr present",
                &module,
                "fs_present",
                &[Some(&layout)],
                HDR,
                None,
            );
            let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("vr present"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vr present"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            Present {
                layout,
                pipeline,
                sampler,
                buf,
            }
        })
    }

    /// Draws `view` of `vol` into `target` (the HDR scene, `size` px) inside its rectangle,
    /// accumulating one more frame when the view is unchanged and not yet final. `bricks`
    /// (of the complete volume) enables empty-space skipping once every slice is on the GPU.
    /// True while more frames would refine the image.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        vol: &VolumeTexture,
        bricks: Option<&Bricks>,
        view: &View3d,
    ) -> bool {
        let x0 = view.rect[0].min(size.0);
        let y0 = view.rect[1].min(size.1);
        let x1 = view.rect[0].saturating_add(view.rect[2]).min(size.0);
        let y1 = view.rect[1].saturating_add(view.rect[3]).min(size.1);
        if x1 <= x0 || y1 <= y0 || view.rect[2] == 0 || view.rect[3] == 0 {
            return false;
        }
        self.pipe(gpu, vol.kind);
        let s = setup(&vol.geometry, vol.table, view);
        let transfer = view.transfer();
        let tf_key = (
            view.preset,
            [view.series.0.to_bits(), view.series.1.to_bits()],
        );
        if self.tf.as_ref().is_none_or(|(k, _)| *k != tf_key) {
            self.tf = Some((tf_key, tf_texture(gpu, &transfer)));
            self.generation += 1;
        }
        // Bricks apply to the complete volume they were computed for.
        let grid = vol.geometry.dims.map(|d| d.div_ceil(BRICK).max(1));
        let bricks = bricks.filter(|b| {
            b.grid == grid && vol.uploaded() == vol.geometry.slices() && !b.range.is_empty()
        });
        if let Some(b) = bricks {
            let key = (
                b.id,
                tf_key,
                [view.tf_window.0.to_bits(), view.tf_window.1.to_bits()],
            );
            if self.bricks.as_ref().is_none_or(|(k, _)| *k != key) {
                let lut = transfer.lut();
                let rows = b.rows(occupancy(&lut, s.mapping));
                let fits = self
                    .bricks
                    .as_ref()
                    .is_some_and(|(k, buf)| k.0 == b.id && buf.size() == (rows.len() * 16) as u64);
                if !fits {
                    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("vr bricks"),
                        size: (rows.len() * 16) as u64,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    self.bricks = Some((key, buf));
                    self.generation += 1;
                }
                if let Some((k, buf)) = &mut self.bricks {
                    *k = key;
                    gpu.queue.write_buffer(buf, 0, bytemuck::cast_slice(&rows));
                }
            }
        }
        let dummy = self.dummy.get_or_insert_with(|| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vr no bricks"),
                size: 16,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            })
        });
        let uniform = self.uniform.get_or_insert_with(|| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vr"),
                size: (ROWS * 16) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let size_acc = view.accum_size();
        if self.accum.as_ref().is_none_or(|a| a.size != size_acc) {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vr accumulation"),
                size: wgpu::Extent3d {
                    width: size_acc.0,
                    height: size_acc.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.accum = Some(Accum {
                size: size_acc,
                view: texture.create_view(&Default::default()),
            });
            self.generation += 1;
            self.key = 0;
        }
        let used = bricks.map(|b| b.grid);
        let first = uniforms(&s, vol, view, used, 0);
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::hash::DefaultHasher::new();
            bytemuck::cast_slice::<[f32; 4], u32>(&first).hash(&mut h);
            (vol.id, vol.uploaded(), tf_key, view.rect, self.generation).hash(&mut h);
            h.finish() | 1
        };
        if key != self.key {
            self.key = key;
            self.frame = 0;
        }
        let frames = view.frames();
        let pipe = self.pipes[variant(vol.kind)]
            .as_ref()
            .expect("pipeline made above");
        let accum = self.accum.as_ref().expect("made above");
        if self.frame < frames {
            let u = if self.frame == 0 {
                first
            } else {
                uniforms(&s, vol, view, used, self.frame)
            };
            gpu.queue.write_buffer(uniform, 0, bytemuck::cast_slice(&u));
            let gkey = (vol.id, self.generation);
            if self.group.as_ref().is_none_or(|(k, _)| *k != gkey) {
                let tf = &self.tf.as_ref().expect("made above").1;
                let bricks_buf = match (&self.bricks, bricks) {
                    (Some((_, b)), Some(_)) => b,
                    _ => &*dummy,
                };
                let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("vr"),
                    layout: &pipe.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&vol.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: vol.lut.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(tf),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: bricks_buf.as_entire_binding(),
                        },
                    ],
                });
                self.group = Some((gkey, group));
            }
            let w = 1.0 / f64::from(self.frame + 1);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vr"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &accum.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if self.frame == 0 {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipe.pipeline);
            pass.set_blend_constant(wgpu::Color {
                r: w,
                g: w,
                b: w,
                a: w,
            });
            pass.set_bind_group(0, &self.group.as_ref().expect("made above").1, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            self.frame += 1;
        }
        let generation = self.generation;
        let rect = [x0, y0, x1 - x0, y1 - y0];
        let p = [
            view.rect[0] as f32,
            view.rect[1] as f32,
            view.rect[2] as f32,
            view.rect[3] as f32,
        ];
        let canvas = view.canvas;
        self.present(gpu);
        let present = self.present.as_ref().expect("made above");
        gpu.queue
            .write_buffer(&present.buf, 0, bytemuck::cast_slice(&[p, canvas]));
        let accum = self.accum.as_ref().expect("made above");
        if self
            .present_group
            .as_ref()
            .is_none_or(|(g, _)| *g != generation)
        {
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vr present"),
                layout: &present.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: present.buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&accum.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&present.sampler),
                    },
                ],
            });
            self.present_group = Some((generation, group));
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("vr present"),
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
        pass.set_pipeline(&present.pipeline);
        pass.set_scissor_rect(rect[0], rect[1], rect[2], rect[3]);
        pass.set_bind_group(0, &self.present_group.as_ref().expect("made above").1, &[]);
        pass.draw(0..3, 0..1);
        self.frame < frames
    }
}

fn entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    }
}

fn tf_texture(gpu: &Gpu, transfer: &Transfer) -> wgpu::TextureView {
    let lut = transfer.lut();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vr transfer"),
        size: wgpu::Extent3d {
            width: LUT_SIZE as u32,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D1,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&lut),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(LUT_SIZE as u32 * 16),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: LUT_SIZE as u32,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    texture.create_view(&Default::default())
}

/// Whether the transfer table shows anything for values in [lo, hi] (one entry of margin
/// either side for rounding).
fn occupancy(lut: &[[f32; 4]], mapping: (f64, f64)) -> impl Fn(f32, f32) -> bool {
    let mut count = vec![0u32; lut.len() + 1];
    for (i, e) in lut.iter().enumerate() {
        count[i + 1] = count[i] + u32::from(e[3] > 0.0);
    }
    let n = lut.len() as f64;
    move |lo: f32, hi: f32| {
        let a = f64::from(lo) * mapping.0 + mapping.1;
        let b = f64::from(hi) * mapping.0 + mapping.1;
        let i0 = (a.min(b).floor() - 1.0).clamp(0.0, n - 1.0) as usize;
        let i1 = (a.max(b).ceil() + 1.0).clamp(0.0, n - 1.0) as usize;
        count[i1 + 1] > count[i0]
    }
}

/// The volume rendering shader for one texture type (`f32`, `i32` or `u32`).
pub fn shader_source(sample: &str) -> String {
    with_sampler(include_str!("shaders/vr.wgsl"), sample)
}

/// The shader that shows the accumulation.
pub fn present_source() -> String {
    format!(
        "{FULLSCREEN_WGSL}\n{}",
        include_str!("shaders/present.wgsl")
    )
}

#[cfg(test)]
#[path = "vr_tests.rs"]
mod tests;
