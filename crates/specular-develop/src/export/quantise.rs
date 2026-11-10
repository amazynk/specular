//! 8- and 16-bit export samples made on the GPU, the same codes as
//! the CPU's [`Curve`](super::render) path: a band is read back as packed RGB codes (3 or 6
//! bytes per pixel instead of 16), and the host only copies them.
//!
//! The CPU's code of a linear value `l` is `(curve(l) · max + 0.5)` as an integer (`max`
//! 255 or 65535), where `curve` is the tabulated transfer (a square root of a square root,
//! an index and a linear interpolation). A GPU's square root need not be correctly
//! rounded, so the curve is not evaluated there. Instead the host finds, for each code `k`,
//! the smallest f32 whose code is at least `k` (a binary search over the bit patterns of
//! 0..=1, whose order is the order of the values), and the GPU counts the thresholds at or
//! below each value (a binary search of its own): exact whenever the curve never decreases
//! over the f32 values of 0..=1, which `the_curves_never_decrease` checks for every export
//! space's curve, value by value (a code is a rounding of the curve, so it never decreases
//! either). Values below 0, above 1 and NaN get the CPU's codes for them (0's, 1's and
//! NaN's).

use rayon::prelude::*;

/// Thresholds of the codes above 0 (`2^bits − 1` of them, padded to `2^bits`; `−∞` for
/// codes 0 already has, `+∞` for codes 1 never reaches) and the code of NaN.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Thresholds {
    pub(super) t: Vec<f32>,
    pub(super) nan: u16,
    /// 8 or 16.
    pub(super) bits: u32,
}

impl Thresholds {
    /// For the `bits`-bit code function `code` (monotone over the f32 values of 0..=1).
    pub(super) fn new(bits: u32, code: impl Fn(f32) -> u16 + Sync) -> Thresholds {
        let lo = u32::from(code(0.0));
        let hi = u32::from(code(1.0));
        let n = 1u32 << bits;
        let one = 1f32.to_bits();
        let t = (1..=n)
            .into_par_iter()
            .map(|k| {
                if k >= n || hi < k {
                    f32::INFINITY
                } else if lo >= k {
                    f32::NEG_INFINITY
                } else {
                    // The smallest bit pattern in (0, 1] whose code reaches k.
                    let (mut a, mut b) = (0u32, one);
                    while a + 1 < b {
                        let m = a + (b - a) / 2;
                        if u32::from(code(f32::from_bits(m))) >= k {
                            b = m;
                        } else {
                            a = m;
                        }
                    }
                    f32::from_bits(b)
                }
            })
            .collect();
        Thresholds {
            t,
            nan: code(f32::NAN),
            bits,
        }
    }

    /// The code of `l` as the GPU computes it (tests).
    #[cfg(test)]
    pub(super) fn code(&self, l: f32) -> u16 {
        if l.is_nan() {
            return self.nan;
        }
        let v = l.min(1.0);
        let mut c = 0usize;
        let mut step = 1usize << (self.bits - 1);
        while step > 0 {
            if self.t[c + step - 1] <= v {
                c += step;
            }
            step >>= 1;
        }
        c as u16
    }
}

const WGSL: &str = "
@group(0) @binding(0) var<storage, read> src: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<storage, read> T: array<f32>;
struct P { words: u32, samples: u32, groups_x: u32, nan_code: u32, bits: u32 }
@group(0) @binding(3) var<uniform> prm: P;

fn code(l: f32) -> u32 {
    // NaN by its bits (a compiler may assume no NaN in comparisons).
    if ((bitcast<u32>(l) & 0x7fffffffu) > 0x7f800000u) { return prm.nan_code; }
    let v = min(l, 1.0);
    var c = 0u;
    for (var step = 1u << (prm.bits - 1u); step > 0u; step >>= 1u) {
        if (T[c + step - 1u] <= v) { c += step; }
    }
    return c;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    let wi = g.y * prm.groups_x * 256u + g.x;
    if (wi >= prm.words) { return; }
    // 4 codes a word (8 bits) or 2 (16 bits).
    let per = 32u / prm.bits;
    var word = 0u;
    for (var b = 0u; b < per; b++) {
        let s = wi * per + b;
        if (s < prm.samples) {
            word |= code(src[(s / 3u) * 4u + s % 3u]) << (prm.bits * b);
        }
    }
    dst[wi] = word;
}
";

/// The GPU side: one pipeline; per band an output buffer per slot.
pub(super) struct Quantiser {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    /// Packed bytes per read-back slot (reused while large enough).
    out: Vec<Option<wgpu::Buffer>>,
}

/// A curve's thresholds on the GPU.
#[derive(Clone)]
pub(super) struct GpuThresholds {
    buf: wgpu::Buffer,
    nan: u16,
    /// 8 or 16.
    pub(super) bits: u32,
}

impl Quantiser {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Quantiser {
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
            label: Some("develop export quantise"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(3, wgpu::BufferBindingType::Uniform),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop export quantise"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop export quantise"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("develop export quantise"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Quantiser {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipeline,
            out: Vec::new(),
        }
    }

    pub(super) fn thresholds(&self, t: &Thresholds) -> GpuThresholds {
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop export thresholds"),
            size: 4 * t.t.len() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buf, 0, bytemuck::cast_slice(&t.t));
        GpuThresholds {
            buf,
            nan: t.nan,
            bits: t.bits,
        }
    }

    /// Records the codes of the first `pixels` RGBA f32 pixels of `src` as packed RGB codes
    /// (bytes, or little-endian 16-bit halves of words) into slot `slot`'s buffer (made or
    /// grown here), returned with its byte count (a multiple of 4).
    pub(super) fn record(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        src: &wgpu::Buffer,
        pixels: u64,
        t: &GpuThresholds,
        slot: usize,
    ) -> (wgpu::Buffer, u64) {
        let samples = pixels * 3;
        let words = samples.div_ceil(u64::from(32 / t.bits));
        let bytes = (words * 4).max(16);
        if self.out.len() <= slot {
            self.out.resize(slot + 1, None);
        }
        let out = match &self.out[slot] {
            Some(b) if b.size() >= bytes => b.clone(),
            _ => {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("develop export bytes"),
                    size: bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                self.out[slot] = Some(b.clone());
                b
            }
        };
        let groups = words.div_ceil(256) as u32;
        let gx = groups.clamp(1, 65535);
        let gy = groups.div_ceil(gx).max(1);
        let words32 = words as u32;
        let p: [u32; 8] = [
            words32,
            samples as u32,
            gx,
            u32::from(t.nan),
            t.bits,
            0,
            0,
            0,
        ];
        let pbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop export quantise params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&pbuf, 0, bytemuck::cast_slice(&p));
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("develop export quantise"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: src,
                        offset: 0,
                        size: wgpu::BufferSize::new(pixels * 16),
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
                    resource: t.buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: pbuf.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("develop export quantise"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        (out, bytes)
    }

    /// Drops the kept buffers.
    pub(super) fn release(&mut self) {
        self.out.clear();
    }
}
