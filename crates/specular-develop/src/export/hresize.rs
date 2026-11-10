//! The horizontal half of a resized export's Lanczos-3 ([`super::resample`]) on the GPU: a
//! band is read back as `ow × 3` values per row instead of `iw × 4`, and the host only does
//! the vertical half. Shrinking a 24 MP shot to 2048 px reads back about a quarter of the
//! bytes and leaves the CPU a third of the multiply-adds.
//!
//! The taps are the CPU's (`specular_convert::pixels::taps`), summed in the same order; a
//! GPU's fused multiply-add may round the last bit differently, so the result matches the
//! CPU's to within float rounding, not bit for bit (`gpu_rows_match_the_cpu`).

use specular_convert::pixels::Taps;

const WGSL: &str = "
@group(0) @binding(0) var<storage, read> src: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<f32>;
// Per output column: first input column, tap count, offset into W.
@group(0) @binding(2) var<storage, read> taps: array<vec4<u32>>;
@group(0) @binding(3) var<storage, read> W: array<f32>;
struct P { iw: u32, ow: u32, rows: u32, groups_x: u32 }
@group(0) @binding(4) var<uniform> prm: P;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    let i = g.y * prm.groups_x * 64u + g.x;
    if (i >= prm.ow * prm.rows) { return; }
    let x = i % prm.ow;
    let row = i / prm.ow;
    let t = taps[x];
    var a = vec3<f32>(0.0);
    let base = row * prm.iw;
    for (var j = 0u; j < t.y; j++) {
        let p = (base + t.x + j) * 4u;
        a += W[t.z + j] * vec3<f32>(src[p], src[p + 1u], src[p + 2u]);
    }
    let o = i * 3u;
    dst[o] = a.x;
    dst[o + 1u] = a.y;
    dst[o + 2u] = a.z;
}
";

/// One resize's taps on the GPU, and its pipeline.
pub(super) struct HResize {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    taps: wgpu::Buffer,
    weights: wgpu::Buffer,
    iw: u32,
    ow: u32,
    /// Output per read-back slot (reused while large enough).
    out: Vec<Option<wgpu::Buffer>>,
}

impl HResize {
    /// For rows `iw` wide resized to `hx.len()` columns.
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, iw: u32, hx: &[Taps]) -> Self {
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
            label: Some("develop export resize"),
            entries: &[
                entry(0, ro),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, ro),
                entry(3, ro),
                entry(4, wgpu::BufferBindingType::Uniform),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop export resize"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop export resize"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("develop export resize"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let (mut t, mut w) = (Vec::with_capacity(hx.len() * 4), Vec::new());
        for tap in hx {
            t.extend([tap.start as u32, tap.w.len() as u32, w.len() as u32, 0]);
            w.extend_from_slice(&tap.w);
        }
        if w.is_empty() {
            w.push(0.0);
        }
        let upload = |label, data: &[u8]| {
            let b = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (data.len() as u64).max(16).div_ceil(4) * 4,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&b, 0, data);
            b
        };
        HResize {
            taps: upload("develop export resize taps", bytemuck::cast_slice(&t)),
            weights: upload("develop export resize weights", bytemuck::cast_slice(&w)),
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipeline,
            iw,
            ow: hx.len() as u32,
            out: Vec::new(),
        }
    }

    /// Records the horizontal resize of `rows` RGBA f32 rows of `src` into slot `slot`'s
    /// buffer, returned with its byte count (`rows × ow × 3` floats).
    pub(super) fn record(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        src: &wgpu::Buffer,
        rows: u32,
        slot: usize,
    ) -> (wgpu::Buffer, u64) {
        let n = u64::from(self.ow) * u64::from(rows);
        let bytes = (n * 12).max(16);
        if self.out.len() <= slot {
            self.out.resize(slot + 1, None);
        }
        let out = match &self.out[slot] {
            Some(b) if b.size() >= bytes => b.clone(),
            _ => {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("develop export resized"),
                    size: bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                self.out[slot] = Some(b.clone());
                b
            }
        };
        let groups = n.div_ceil(64) as u32;
        let gx = groups.clamp(1, 65535);
        let gy = groups.div_ceil(gx).max(1);
        let p: [u32; 4] = [self.iw, self.ow, rows, gx];
        let pbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop export resize params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&pbuf, 0, bytemuck::cast_slice(&p));
        let input = u64::from(self.iw) * u64::from(rows) * 16;
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("develop export resize"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: src,
                        offset: 0,
                        size: wgpu::BufferSize::new(input),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &out,
                        offset: 0,
                        size: wgpu::BufferSize::new(bytes),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.taps.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.weights.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: pbuf.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("develop export resize"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        (out, n * 12)
    }
}
