//! The fused warp on the GPU: one compute pass that maps every output pixel back through
//! [`Plan`] and samples the source once (Catmull-Rom, 4×4 taps, edges clamped as the CPU
//! DNG develop does; per plane when the planes move apart). Points that leave the source by
//! more than a pixel are black with alpha 0.
//!
//! [`params`] lays the plan out as one f32 storage buffer; [`cpu_ref`] (tests) does the same
//! arithmetic on the CPU.

use crate::gpu::Error;
use crate::graph::Precision;

use super::lens::{RHO_MAX, TABLE_N};
use super::{DngStep, Plan};

/// Header words before the tables.
const HEADER: usize = 64;
/// Words per DNG step.
const STEP_WORDS: usize = 24;
/// Most DNG steps the kernel runs.
pub const MAX_DNG: usize = 8;

const F_PER_PLANE: u32 = 1;
const F_MATRIX: u32 = 2;
const F_TABLES: u32 = 4;
/// Perspective: words 50..59 hold [`Plan::persp`] (rows) instead of the turn.
const F_PERSP: u32 = 8;

/// Output rows per dispatch band (each band binds its own slice of the output).
const WG: u32 = 16;

/// The plan's words for an `out_w × out_h` output from an `sw × sh` source.
pub fn params(
    plan: &Plan,
    sw: u32,
    sh: u32,
    in_prec: Precision,
    out_w: u32,
    out_h: u32,
    out_prec: Precision,
) -> Vec<f32> {
    let tables = !plan.radial.is_identity();
    let dng: Vec<&DngStep> = plan.dng.iter().take(MAX_DNG).collect();
    let n = HEADER + if tables { 4 * TABLE_N } else { 0 } + dng.len() * STEP_WORDS;
    let mut p = vec![0f32; n];
    let u = |v: u32| f32::from_bits(v);
    let [x0, y0, cw, ch] = plan.crop_rect();
    let [fx, fy, fw, fh] = plan.frame;
    let (wo, ho) = plan.oriented;
    let m = plan.corrected_matrix();
    let mut flags = 0;
    if plan.per_plane() {
        flags |= F_PER_PLANE;
    }
    if plan.per_plane() && plan.camera.is_some() {
        flags |= F_MATRIX;
    }
    if tables {
        flags |= F_TABLES;
    }
    if plan.persp.is_some() {
        flags |= F_PERSP;
    }
    let (s, c) = plan.angle.sin_cos();
    let head: [f32; 32] = [
        u(out_w),
        u(out_h),
        u(sw),
        u(sh),
        u(u32::from(in_prec == Precision::F16)),
        u(u32::from(out_prec == Precision::F16)),
        u(flags),
        u(dng.len() as u32),
        x0 as f32,
        y0 as f32,
        (cw / f64::from(out_w)) as f32,
        (ch / f64::from(out_h)) as f32,
        c as f32,
        s as f32,
        (wo / 2.0) as f32,
        (ho / 2.0) as f32,
        m[0][0] as f32,
        m[0][1] as f32,
        m[1][0] as f32,
        m[1][1] as f32,
        (fw / 2.0) as f32,
        (fh / 2.0) as f32,
        (2.0 / (fw * fw + fh * fh).sqrt()) as f32,
        (f64::from(sw) / fw) as f32,
        (f64::from(sh) / fh) as f32,
        u(HEADER as u32),
        u(TABLE_N as u32),
        RHO_MAX as f32,
        u((HEADER + if tables { 4 * TABLE_N } else { 0 }) as u32),
        0.0,
        0.0,
        0.0,
    ];
    p[..32].copy_from_slice(&head);
    if let Some(mm) = plan.camera {
        let inv = inv3(&mm);
        for i in 0..9 {
            p[32 + i] = mm[i / 3][i % 3];
            p[41 + i] = inv[i / 3][i % 3];
        }
    }
    if let Some(g) = plan.persp {
        for i in 0..9 {
            p[50 + i] = g[i / 3][i % 3] as f32;
        }
    }
    let mut at = HEADER;
    if tables {
        let t = plan.radial.tables();
        p[at..at + t.len()].copy_from_slice(&t);
        at += t.len();
    }
    let (ccx, ccy) = (fx + fw / 2.0, fy + fh / 2.0);
    for st in dng {
        let w = &mut p[at..at + STEP_WORDS];
        match st {
            DngStep::Warp {
                fisheye,
                k,
                centre,
                m,
            } => {
                w[0] = u(if *fisheye { 1 } else { 0 });
                w[1] = (centre.0 - ccx) as f32;
                w[2] = (centre.1 - ccy) as f32;
                w[3] = *m as f32;
                for (i, v) in k.iter().flatten().enumerate() {
                    w[4 + i] = *v as f32;
                }
            }
            DngStep::Vignette { k, centre, inv_m2 } => {
                w[0] = u(2);
                w[1] = (centre.0 - ccx) as f32;
                w[2] = (centre.1 - ccy) as f32;
                w[3] = *inv_m2 as f32;
                for (i, v) in k.iter().enumerate() {
                    w[4 + i] = *v as f32;
                }
            }
        }
        at += STEP_WORDS;
    }
    p
}

/// Inverse of a 3×3 matrix (identity when singular).
fn inv3(m: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let a: [[f64; 3]; 3] = m.map(|r| r.map(f64::from));
    match crate::colour::inv(&a) {
        Some(i) => i.map(|r| r.map(|v| v as f32)),
        None => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    }
}

pub const WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> P: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<storage, read> src: array<u32>;
@group(0) @binding(3) var<uniform> band: vec4<u32>;

fn pu(i: u32) -> u32 { return bitcast<u32>(P[i]); }

fn load(x: i32, y: i32) -> vec4<f32> {
    let sw = i32(pu(2u));
    let sh = i32(pu(3u));
    let i = u32(clamp(y, 0, sh - 1) * sw + clamp(x, 0, sw - 1));
    if (pu(4u) == 1u) {
        return vec4<f32>(unpack2x16float(src[2u * i]), unpack2x16float(src[2u * i + 1u]));
    }
    return vec4<f32>(bitcast<f32>(src[4u * i]), bitcast<f32>(src[4u * i + 1u]),
        bitcast<f32>(src[4u * i + 2u]), bitcast<f32>(src[4u * i + 3u]));
}

fn cubic(t: f32) -> vec4<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return vec4<f32>(-0.5 * t3 + t2 - 0.5 * t, 1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t, 0.5 * t3 - 0.5 * t2);
}

// Catmull-Rom at buffer coordinates (pixel centres at integers).
fn cr_at(b: vec2<f32>) -> vec4<f32> {
    let f = floor(b);
    let wx = cubic(b.x - f.x);
    let wy = cubic(b.y - f.y);
    let x = i32(f.x);
    let y = i32(f.y);
    var acc = vec4<f32>(0.0);
    for (var j = 0; j < 4; j++) {
        let row = load(x - 1, y - 1 + j) * wx.x + load(x, y - 1 + j) * wx.y
            + load(x + 1, y - 1 + j) * wx.z + load(x + 2, y - 1 + j) * wx.w;
        acc += row * wy[j];
    }
    return acc;
}

fn table(t: u32, rho: f32) -> f32 {
    let n = pu(26u);
    let x = clamp(rho / P[27u] * f32(n - 1u), 0.0, f32(n - 1u));
    let i = min(u32(floor(x)), n - 2u);
    let f = x - f32(i);
    let base = pu(25u) + t * n;
    return P[base + i] * (1.0 - f) + P[base + i + 1u] * f;
}

// atan(x) for x >= 0 to about 1e-7 relative (built-in atan is less exact on some GPUs):
// 1/x above 1, one half-angle step, then the odd series to x^17.
fn atan_p(x: f32) -> f32 {
    var a = x;
    let inv = a > 1.0;
    if (inv) { a = 1.0 / a; }
    let h = a / (1.0 + sqrt(1.0 + a * a));
    let h2 = h * h;
    let s = 1.0 - h2 * (1.0 / 3.0 - h2 * (1.0 / 5.0 - h2 * (1.0 / 7.0 - h2 * (1.0 / 9.0
        - h2 * (1.0 / 11.0 - h2 * (1.0 / 13.0 - h2 * (1.0 / 15.0 - h2 / 17.0)))))));
    var t = 2.0 * h * s;
    if (inv) { t = 1.5707963267948966 - t; }
    return t;
}

// Back-maps a corrected point (relative to the frame centre) for plane `p`: the source
// point (relative) and the gain.
fn to_source(rel: vec2<f32>, p: u32) -> vec3<f32> {
    var q = rel;
    var gain = 1.0;
    let inv_r = P[22u];
    if ((pu(6u) & 4u) != 0u) {
        q = rel * table(p, length(rel) * inv_r);
    }
    let n = pu(7u);
    let base = pu(28u);
    for (var s = i32(n) - 1; s >= 0; s--) {
        let o = base + u32(s) * 24u;
        let kind = pu(o);
        let c = vec2<f32>(P[o + 1u], P[o + 2u]);
        if (kind == 2u) {
            let d = q - c;
            let r2 = dot(d, d) * P[o + 3u];
            var acc = 0.0;
            var rp = r2;
            for (var i = 0u; i < 5u; i++) {
                acc += P[o + 4u + i] * rp;
                rp *= r2;
            }
            gain *= 1.0 + acc;
        } else {
            let m = P[o + 3u];
            let k = o + 4u + p * 6u;
            let d = (q - c) / m;
            let r2 = dot(d, d);
            var fxy: vec2<f32>;
            if (kind == 1u) {
                let r = sqrt(r2);
                if (r < 1e-12) {
                    fxy = P[k] * d;
                } else {
                    let t = atan_p(r);
                    let t2 = t * t;
                    let rd = t * (P[k] + t2 * (P[k + 1u] + t2 * (P[k + 2u] + t2 * P[k + 3u])));
                    fxy = rd / r * d;
                }
            } else {
                let f = P[k] + r2 * (P[k + 1u] + r2 * (P[k + 2u] + r2 * P[k + 3u]));
                let dxy = 2.0 * d.x * d.y;
                fxy = vec2<f32>(
                    f * d.x + P[k + 4u] * dxy + P[k + 5u] * (r2 + 2.0 * d.x * d.x),
                    f * d.y + P[k + 5u] * dxy + P[k + 4u] * (r2 + 2.0 * d.y * d.y));
            }
            q = c + m * fxy;
        }
    }
    if ((pu(6u) & 4u) != 0u) {
        gain *= table(3u, length(q) * inv_r);
    }
    return vec3<f32>(q, gain);
}

fn buffer_xy(q: vec2<f32>) -> vec2<f32> {
    return (q + vec2<f32>(P[20u], P[21u])) * vec2<f32>(P[23u], P[24u]) - 0.5;
}

fn mul3(o: u32, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        P[o] * v.x + P[o + 1u] * v.y + P[o + 2u] * v.z,
        P[o + 3u] * v.x + P[o + 4u] * v.y + P[o + 5u] * v.z,
        P[o + 6u] * v.x + P[o + 7u] * v.y + P[o + 8u] * v.z);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let ow = pu(0u);
    let y = gid.y + band.x;
    if (gid.x >= ow || y >= band.y) { return; }
    // Output pixel → straightened frame → oriented frame (relative to its centre).
    let f = vec2<f32>(P[8u] + (f32(gid.x) + 0.5) * P[10u], P[9u] + (f32(y) + 0.5) * P[11u]);
    let d = f - vec2<f32>(P[14u], P[15u]);
    var o: vec2<f32>;
    var seen = true;
    if ((pu(6u) & 8u) != 0u) {
        // Perspective: the homography (straighten included), divided through.
        let q = vec3<f32>(P[50u] * d.x + P[51u] * d.y + P[52u],
            P[53u] * d.x + P[54u] * d.y + P[55u],
            P[56u] * d.x + P[57u] * d.y + P[58u]);
        seen = q.z > 1e-12;
        o = q.xy / max(q.z, 1e-12);
    } else {
        let c = P[12u];
        let s = P[13u];
        o = vec2<f32>(d.x * c + d.y * s, -d.x * s + d.y * c);
    }
    // Oriented → corrected (relative to the frame centre).
    let rel = vec2<f32>(P[16u] * o.x + P[17u] * o.y, P[18u] * o.x + P[19u] * o.y);
    let g = to_source(rel, 1u);
    let bg = buffer_xy(g.xy);
    let sw = f32(pu(2u));
    let sh = f32(pu(3u));
    var res = vec4<f32>(0.0);
    if (seen && bg.x >= -1.0 && bg.y >= -1.0 && bg.x <= sw && bg.y <= sh) {
        let flags = pu(6u);
        let sg = cr_at(bg);
        if ((flags & 1u) == 0u) {
            res = vec4<f32>(sg.rgb * g.z, 1.0);
        } else {
            var v = vec3<f32>(0.0);
            for (var p = 0u; p < 3u; p++) {
                var t = g;
                var sp = sg;
                if (p != 1u) {
                    t = to_source(rel, p);
                    sp = cr_at(buffer_xy(t.xy));
                }
                var cam = sp.rgb;
                if ((flags & 2u) != 0u) {
                    cam = mul3(41u, sp.rgb);
                }
                v[p] = cam[p] * t.z;
            }
            if ((flags & 2u) != 0u) {
                v = mul3(32u, v);
            }
            res = vec4<f32>(v, 1.0);
        }
    }
    let i = (y - band.x) * ow + gid.x;
    if (pu(5u) == 1u) {
        dst[2u * i] = pack2x16float(res.xy);
        dst[2u * i + 1u] = pack2x16float(res.zw);
    } else {
        dst[4u * i] = bitcast<u32>(res.x);
        dst[4u * i + 1u] = bitcast<u32>(res.y);
        dst[4u * i + 2u] = bitcast<u32>(res.z);
        dst[4u * i + 3u] = bitcast<u32>(res.w);
    }
}
"#;

/// The warp pass on one device.
pub struct Warper {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    max_binding: u64,
    max_buffer: u64,
    align: u64,
    /// An output buffer given back ([`Warper::give_back`]), reused when large enough.
    spare: Option<wgpu::Buffer>,
    /// The params buffer and the band uniforms, kept between runs.
    params: Option<wgpu::Buffer>,
    bands: Vec<wgpu::Buffer>,
    /// Passes run (tests count them).
    pub runs: u64,
    /// Output buffers made (not reused).
    pub buffers_created: u64,
}

impl Warper {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Warper {
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
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("develop warp"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(3, wgpu::BufferBindingType::Uniform),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop warp"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop warp"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("develop warp"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let limits = device.limits();
        Warper {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipeline,
            max_binding: limits
                .max_storage_buffer_binding_size
                .min(limits.max_buffer_size),
            max_buffer: limits.max_buffer_size,
            align: u64::from(limits.min_storage_buffer_offset_alignment).max(16),
            spare: None,
            params: None,
            bands: Vec::new(),
            runs: 0,
            buffers_created: 0,
        }
    }

    /// Gives an earlier output back for the next run to write into (when nothing else uses
    /// it any more; later GPU work on the queue is ordered after earlier reads).
    pub fn give_back(&mut self, buf: wgpu::Buffer) {
        if self.spare.as_ref().is_none_or(|s| s.size() < buf.size()) {
            self.spare = Some(buf);
        }
    }

    /// Drops the kept buffers (a new image).
    pub fn release(&mut self) {
        self.spare = None;
        self.params = None;
        self.bands.clear();
    }

    /// Warps the packed `sw × sh` source `src` into a new `out_w × out_h` buffer (rows
    /// packed, `out_prec`, usage `STORAGE | COPY_SRC | COPY_DST`). Submits; does not wait.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        plan: &Plan,
        src: &wgpu::Buffer,
        sw: u32,
        sh: u32,
        in_prec: Precision,
        out_w: u32,
        out_h: u32,
        out_prec: Precision,
    ) -> Result<wgpu::Buffer, Error> {
        let src_bytes = u64::from(sw) * u64::from(sh) * in_prec.bytes_per_pixel();
        let bpp = out_prec.bytes_per_pixel();
        let out_bytes = u64::from(out_w) * u64::from(out_h) * bpp;
        if src_bytes > self.max_binding || src.size() < src_bytes {
            return Err(Error::TooLarge { needs: src_bytes });
        }
        if out_bytes > self.max_buffer {
            return Err(Error::TooLarge { needs: out_bytes });
        }
        let words = params(plan, sw, sh, in_prec, out_w, out_h, out_prec);
        let device = self.device.clone();
        let pbytes = (words.len() as u64 * 4).max(16);
        let pbuf = match self.params.take() {
            Some(b) if b.size() >= pbytes => b,
            _ => device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop warp params"),
                size: pbytes.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        };
        self.queue
            .write_buffer(&pbuf, 0, bytemuck::cast_slice(&words));
        let dst_bytes = out_bytes.max(16).div_ceil(16) * 16;
        let dst = match self.spare.take() {
            Some(b) if b.size() >= dst_bytes => b,
            _ => {
                self.buffers_created += 1;
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("develop warped source"),
                    size: dst_bytes,
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_SRC
                        | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            }
        };
        // Bands of rows whose slice fits one binding and starts aligned.
        let row = u64::from(out_w) * bpp;
        let mut rows = (self.max_binding / row.max(1)).clamp(1, u64::from(out_h)) as u32;
        while rows > 1 && !(u64::from(rows) * row).is_multiple_of(self.align) {
            rows -= 1;
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("develop warp"),
        });
        let mut y = 0;
        let mut band = 0;
        while y < out_h {
            let n = rows.min(out_h - y);
            if self.bands.len() <= band {
                self.bands
                    .push(device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("develop warp band"),
                        size: 16,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
            }
            let ub = self.bands[band].clone();
            band += 1;
            self.queue
                .write_buffer(&ub, 0, bytemuck::cast_slice(&[y, y + n, 0u32, 0]));
            let offset = u64::from(y) * row;
            let size = (u64::from(n) * row).div_ceil(4) * 4;
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("develop warp"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: pbuf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &dst,
                            offset,
                            size: std::num::NonZeroU64::new(size),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: src,
                            offset: 0,
                            size: std::num::NonZeroU64::new(src_bytes.div_ceil(4) * 4),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: ub.as_entire_binding(),
                    },
                ],
            });
            {
                let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("develop warp"),
                    timestamp_writes: None,
                });
                cp.set_pipeline(&self.pipeline);
                cp.set_bind_group(0, &bind, &[]);
                cp.dispatch_workgroups(out_w.div_ceil(WG), n.div_ceil(WG), 1);
            }
            y += n;
        }
        self.queue.submit([enc.finish()]);
        self.params = Some(pbuf);
        self.runs += 1;
        Ok(dst)
    }
}

/// The kernel's arithmetic on the CPU (f64), for tests: `src` is `sw × sh` RGBA f32; returns
/// `out_w × out_h` RGBA.
#[cfg(test)]
pub fn cpu_ref(plan: &Plan, src: &[f32], sw: u32, sh: u32, out_w: u32, out_h: u32) -> Vec<f32> {
    let [x0, y0, cw, ch] = plan.crop_rect();
    let [fx, fy, fw, fh] = plan.frame;
    let (kx, ky) = (cw / f64::from(out_w), ch / f64::from(out_h));
    let load = |x: i64, y: i64| -> [f64; 4] {
        let x = x.clamp(0, i64::from(sw) - 1) as usize;
        let y = y.clamp(0, i64::from(sh) - 1) as usize;
        let i = (y * sw as usize + x) * 4;
        std::array::from_fn(|c| f64::from(src[i + c]))
    };
    let cubic = |t: f64| {
        let t2 = t * t;
        let t3 = t2 * t;
        [
            -0.5 * t3 + t2 - 0.5 * t,
            1.5 * t3 - 2.5 * t2 + 1.0,
            -1.5 * t3 + 2.0 * t2 + 0.5 * t,
            0.5 * t3 - 0.5 * t2,
        ]
    };
    let sample = |bx: f64, by: f64| -> [f64; 4] {
        let (fxl, fyl) = (bx.floor(), by.floor());
        let (wx, wy) = (cubic(bx - fxl), cubic(by - fyl));
        let mut acc = [0f64; 4];
        for (j, wyv) in wy.iter().enumerate() {
            for (i, wxv) in wx.iter().enumerate() {
                let v = load(fxl as i64 - 1 + i as i64, fyl as i64 - 1 + j as i64);
                for c in 0..4 {
                    acc[c] += v[c] * wxv * wyv;
                }
            }
        }
        acc
    };
    let to_buf = |p: (f64, f64)| {
        (
            (p.0 - fx) * f64::from(sw) / fw - 0.5,
            (p.1 - fy) * f64::from(sh) / fh - 0.5,
        )
    };
    let m = plan.camera.map(|m| m.map(|r| r.map(f64::from)));
    let mi = m.and_then(|m| crate::colour::inv(&m));
    let per = plan.per_plane();
    let mut out = vec![0f32; out_w as usize * out_h as usize * 4];
    for oy in 0..out_h {
        for ox in 0..out_w {
            let f = (
                x0 + (f64::from(ox) + 0.5) * kx,
                y0 + (f64::from(oy) + 0.5) * ky,
            );
            let (g, gain) = plan.frame_to_source(1, f);
            let b = to_buf(g);
            let o = (oy as usize * out_w as usize + ox as usize) * 4;
            if b.0 < -1.0 || b.1 < -1.0 || b.0 > f64::from(sw) || b.1 > f64::from(sh) {
                continue;
            }
            let sg = sample(b.0, b.1);
            if !per {
                for c in 0..3 {
                    out[o + c] = (sg[c] * gain) as f32;
                }
            } else {
                let mut v = [0f64; 3];
                for (p, vp) in v.iter_mut().enumerate() {
                    let (q, gq) = plan.frame_to_source(p, f);
                    let bq = to_buf(q);
                    let s = sample(bq.0, bq.1);
                    let cam = match mi {
                        Some(mi) => crate::colour::mulv(&mi, [s[0], s[1], s[2]]),
                        None => [s[0], s[1], s[2]],
                    };
                    *vp = cam[p] * gq;
                }
                let v = match m {
                    Some(m) if mi.is_some() => crate::colour::mulv(&m, v),
                    _ => v,
                };
                for c in 0..3 {
                    out[o + c] = v[c] as f32;
                }
            }
            out[o + 3] = 1.0;
        }
    }
    out
}
