//! Clipping overlays: where the picture loses detail, painted
//! over it in a colour per kind. They are drawn over the preview only; exports never see
//! them.
//!
//! - **Display** ([`DISPLAY_HIGH`], [`DISPLAY_LOW`]): after the output transform, a channel
//!   at the top code or every channel at the bottom one (8-bit display codes 255 and 0).
//! - **Scene** ([`SCENE_HIGH`], [`SCENE_LOW`]): before the view transform, the brightest
//!   channel above the scene value the look turns into display white, or below the one it
//!   turns into black ([`scene_limits`]): detail the tone map squeezes into one code.
//! - **Gamut** ([`GAMUT`]): colours outside the output space before gamut compression (a
//!   channel below zero in the output primaries), or, while soft-proofing, outside the proof
//!   profile's gamut (the round-trip ΔE the proof pass leaves in alpha).
//!
//! Where several apply, display wins over scene, scene over gamut. [`cpu`] is the reference
//! for the GPU pass ([`ClipPass`]), which writes RGBA8 rows (transparent where nothing is
//! clipped) for a texture drawn over the picture.

use crate::graph::Precision;
use crate::model::Edit;
use crate::ops::view;

/// Overlay colours (RGBA8, opaque).
pub const DISPLAY_HIGH: [u8; 4] = [255, 40, 40, 255];
pub const DISPLAY_LOW: [u8; 4] = [40, 100, 255, 255];
pub const SCENE_HIGH: [u8; 4] = [255, 160, 0, 255];
pub const SCENE_LOW: [u8; 4] = [0, 220, 230, 255];
pub const GAMUT: [u8; 4] = [130, 255, 60, 255];

/// Display-linear value whose sRGB code rounds to 255 (254.5 / 255 encoded), per unit of
/// peak.
pub const HIGH: f32 = 0.995_46;
/// Display-linear value whose code rounds to 0 (0.5 / 255 encoded).
pub const LOW: f32 = 0.000_151_8;
/// How far below zero (in the output primaries) a channel may go before a colour counts as
/// outside the output gamut.
pub const GAMUT_EPS: f32 = 0.002;

/// Which overlays are on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClipFlags {
    pub scene: bool,
    pub display: bool,
    pub gamut: bool,
}

impl ClipFlags {
    pub fn any(&self) -> bool {
        self.scene || self.display || self.gamut
    }
}

/// What the pass needs besides the pictures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipParams {
    /// Scene values at or above which the look reaches white, at or below which black
    /// ([`scene_limits`]).
    pub scene_white: f32,
    pub scene_black: f32,
    /// The output's peak (1 for SDR).
    pub peak: f32,
    /// The main picture's alpha is the proof gamut ΔE (soft-proofing on).
    pub proof_gamut: bool,
    /// Display-linear Rec.2020 → the output's linear primaries (row-major).
    pub to_output: [f32; 9],
}

/// The scene values the look of `edit` (as shown: its view transform, with the headroom
/// applied) turns into display white and black: the brightest channel at or above the first
/// is clipped white, at or below the second clipped black. White is infinite when the look
/// never reaches it.
pub fn scene_limits(edit: &Edit) -> (f32, f32) {
    let v = view::settings(edit);
    let c = view::Curve::new(&v);
    let (white, black) = (c.white, c.black);
    let f = |x: f64| view::eval(&v, x);
    // The first scene value whose display value reaches `y` (monotone curve; log search).
    let find = |y: f64| -> f64 {
        let (mut lo, mut hi) = (-30.0f64, 20.0f64);
        if f(hi.exp2()) < y {
            return f64::INFINITY;
        }
        if f(lo.exp2()) >= y {
            return 0.0;
        }
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if f(mid.exp2()) >= y {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi.exp2()
    };
    let w = find(black + f64::from(HIGH) * (white - black).max(0.0));
    let b = find(black + f64::from(LOW));
    let to = |x: f64| {
        if x.is_finite() {
            x as f32
        } else {
            f32::INFINITY
        }
    };
    (to(w), to(b))
}

/// The overlay colour of one pixel: `main` the shown picture (display-linear output, alpha
/// the proof ΔE when proofing), `scene` the scene-linear picture before the look, `disp`
/// the display-linear Rec.2020 picture before the output transform.
pub fn classify(
    main: [f32; 4],
    scene: Option<[f32; 3]>,
    disp: Option<[f32; 3]>,
    p: &ClipParams,
    f: ClipFlags,
) -> [u8; 4] {
    let max3 = |v: [f32; 3]| v[0].max(v[1]).max(v[2]);
    if f.display {
        let m = max3([main[0], main[1], main[2]]);
        if m >= HIGH * p.peak {
            return DISPLAY_HIGH;
        }
        if m <= LOW * p.peak {
            return DISPLAY_LOW;
        }
    }
    if f.scene
        && let Some(s) = scene
    {
        let m = max3(s);
        if m >= p.scene_white {
            return SCENE_HIGH;
        }
        if m <= p.scene_black {
            return SCENE_LOW;
        }
    }
    if f.gamut {
        if p.proof_gamut {
            if main[3] > crate::proof::GAMUT_DE {
                return GAMUT;
            }
        } else if let Some(d) = disp {
            let m = &p.to_output;
            let o = [
                m[0] * d[0] + m[1] * d[1] + m[2] * d[2],
                m[3] * d[0] + m[4] * d[1] + m[5] * d[2],
                m[6] * d[0] + m[7] * d[1] + m[8] * d[2],
            ];
            if o[0].min(o[1]).min(o[2]) < -GAMUT_EPS * p.peak.max(1.0) {
                return GAMUT;
            }
        }
    }
    [0; 4]
}

/// The overlay of whole pictures (RGBA f32, `w`×`h`) on the CPU: the reference.
pub fn cpu(
    main: &[f32],
    scene: Option<&[f32]>,
    disp: Option<&[f32]>,
    p: &ClipParams,
    f: ClipFlags,
) -> Vec<u8> {
    let rgb = |v: &[f32], i: usize| [v[4 * i], v[4 * i + 1], v[4 * i + 2]];
    let n = main.len() / 4;
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n {
        let m = [
            main[4 * i],
            main[4 * i + 1],
            main[4 * i + 2],
            main[4 * i + 3],
        ];
        out.extend_from_slice(&classify(
            m,
            scene.map(|s| rgb(s, i)),
            disp.map(|d| rgb(d, i)),
            p,
            f,
        ));
    }
    out
}

const CLIP_WGSL: &str = "
struct U { w: u32, h: u32, stride: u32, flags: u32, pm: u32, ps: u32, pd: u32, pad: u32,
    scene_white: f32, scene_black: f32, peak: f32, eps: f32,
    m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<storage, read> main_img: array<u32>;
@group(0) @binding(3) var<storage, read> scene_img: array<u32>;
@group(0) @binding(4) var<storage, read> disp_img: array<u32>;
fn ld_main(i: u32) -> vec4<f32> {
    if (u.pm == 1u) {
        return vec4<f32>(unpack2x16float(main_img[2u * i]), unpack2x16float(main_img[2u * i + 1u]));
    }
    return vec4<f32>(bitcast<f32>(main_img[4u * i]), bitcast<f32>(main_img[4u * i + 1u]),
        bitcast<f32>(main_img[4u * i + 2u]), bitcast<f32>(main_img[4u * i + 3u]));
}
fn ld_scene(i: u32) -> vec3<f32> {
    if (u.ps == 1u) {
        return vec4<f32>(unpack2x16float(scene_img[2u * i]), unpack2x16float(scene_img[2u * i + 1u])).xyz;
    }
    return vec3<f32>(bitcast<f32>(scene_img[4u * i]), bitcast<f32>(scene_img[4u * i + 1u]),
        bitcast<f32>(scene_img[4u * i + 2u]));
}
fn ld_disp(i: u32) -> vec3<f32> {
    if (u.pd == 1u) {
        return vec4<f32>(unpack2x16float(disp_img[2u * i]), unpack2x16float(disp_img[2u * i + 1u])).xyz;
    }
    return vec3<f32>(bitcast<f32>(disp_img[4u * i]), bitcast<f32>(disp_img[4u * i + 1u]),
        bitcast<f32>(disp_img[4u * i + 2u]));
}
fn rgba(r: u32, g: u32, b: u32) -> u32 {
    return r | (g << 8u) | (b << 16u) | (255u << 24u);
}
@compute @workgroup_size(16, 16, 1)
fn clip(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= u.w || gid.y >= u.h) { return; }
    let i = gid.y * u.w + gid.x;
    var o = 0u;
    var done = false;
    if ((u.flags & 1u) != 0u) {
        let c = ld_main(i);
        let m = max(c.x, max(c.y, c.z));
        if (m >= HIGH * u.peak) { o = rgba(255u, 40u, 40u); done = true; }
        else if (m <= LOW * u.peak) { o = rgba(40u, 100u, 255u); done = true; }
    }
    if (!done && (u.flags & 2u) != 0u) {
        let s = ld_scene(i);
        let m = max(s.x, max(s.y, s.z));
        if (m >= u.scene_white) { o = rgba(255u, 160u, 0u); done = true; }
        else if (m <= u.scene_black) { o = rgba(0u, 220u, 230u); done = true; }
    }
    if (!done && (u.flags & 4u) != 0u) {
        if ((u.flags & 8u) != 0u) {
            if (ld_main(i).w > GAMUT_DE) { o = rgba(130u, 255u, 60u); }
        } else {
            let d = ld_disp(i);
            let v = vec3<f32>(dot(u.m0.xyz, d), dot(u.m1.xyz, d), dot(u.m2.xyz, d));
            if (min(v.x, min(v.y, v.z)) < -u.eps) { o = rgba(130u, 255u, 60u); }
        }
    }
    dst[gid.y * u.stride + gid.x] = o;
}
";

/// The overlay pass on one device.
pub struct ClipPass {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    uniform: wgpu::Buffer,
    rows: Option<wgpu::Buffer>,
}

/// A picture the pass reads: its buffer and precision.
pub type Img<'a> = (&'a wgpu::Buffer, Precision);

impl ClipPass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> ClipPass {
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("develop clipping"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, ro),
                entry(3, ro),
                entry(4, ro),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop clipping"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let src = format!(
            "const HIGH: f32 = {HIGH:?};\nconst LOW: f32 = {LOW:?};\nconst GAMUT_DE: f32 = {:?};\n{CLIP_WGSL}",
            crate::proof::GAMUT_DE
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop clipping"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("develop clipping"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("clip"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop clipping params"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ClipPass {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipeline,
            uniform,
            rows: None,
        }
    }

    /// Writes the overlay of the `w`×`h` pictures into `texture` (`Rgba8Unorm`, `w`×`h`,
    /// usage `COPY_DST`), or into a buffer it returns when `texture` is `None` (tests: rows
    /// of `w.next_multiple_of(64)` pixels).
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        main: Img,
        scene: Option<Img>,
        disp: Option<Img>,
        w: u32,
        h: u32,
        p: &ClipParams,
        f: ClipFlags,
        texture: Option<&wgpu::Texture>,
    ) -> wgpu::Buffer {
        let stride = w.next_multiple_of(64);
        let bytes = u64::from(stride) * u64::from(h) * 4;
        let rows = match &self.rows {
            Some(b) if b.size() >= bytes => b.clone(),
            _ => {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("develop clipping rows"),
                    size: bytes.max(256),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                self.rows = Some(b.clone());
                b
            }
        };
        let flags = u32::from(f.display)
            | u32::from(f.scene && scene.is_some()) << 1
            | u32::from(f.gamut && (p.proof_gamut || disp.is_some())) << 2
            | u32::from(p.proof_gamut) << 3;
        let pr = |x: Option<Img>| u32::from(x.is_some_and(|i| i.1 == Precision::F16));
        let mut u = [0u32; 24];
        u[..8].copy_from_slice(&[
            w,
            h,
            stride,
            flags,
            u32::from(main.1 == Precision::F16),
            pr(scene),
            pr(disp),
            0,
        ]);
        let fin = |v: f32| if v.is_finite() { v } else { f32::MAX };
        u[8] = fin(p.scene_white).to_bits();
        u[9] = p.scene_black.to_bits();
        u[10] = p.peak.to_bits();
        u[11] = (GAMUT_EPS * p.peak.max(1.0)).to_bits();
        for r in 0..3 {
            for c in 0..3 {
                u[12 + r * 4 + c] = p.to_output[r * 3 + c].to_bits();
            }
        }
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&u));
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("develop clipping"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: rows.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: main.0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: scene.unwrap_or(main).0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: disp.unwrap_or(main).0.as_entire_binding(),
                },
            ],
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("develop clipping"),
            });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("develop clipping"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&self.pipeline);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(w.div_ceil(16), h.div_ceil(16), 1);
        }
        if let Some(t) = texture {
            enc.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &rows,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride * 4),
                        rows_per_image: Some(h),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: t,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.queue.submit([enc.finish()]);
        rows
    }
}

#[cfg(test)]
mod tests;
