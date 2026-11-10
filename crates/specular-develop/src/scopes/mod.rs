//! Scopes: histogram, waveform (luma and RGB),
//! RGB parade and vectorscope of the picture as shown.
//!
//! - **Input.** The rendered picture (display-linear, the output space's primaries, values
//!   up to the peak), sampled on a whole-pixel grid so that about one megapixel counts
//!   ([`stride`]): the navigator-sized view the plan asks for, without a second render. It
//!   runs once per render, so once per edit change, and every scope comes from one pass.
//! - **Levels** are the display encoding (the sRGB curve of value / peak), as a video
//!   waveform shows code values; luma is Rec.709 luminance encoded the same way.
//! - **Vectorscope** in OkLab a/b (a uniform colour space: equal distances look about equally
//!   different, unlike Y′CbCr), ±[`VEC_RANGE`] across. [`targets`] marks the output gamut's
//!   primaries and secondaries, and [`skin_hue`] the skin-tone line.
//! - **One pass, one buffer.** Every bin is a `u32` in one storage buffer ([`Layout`]); a
//!   workgroup keeps its histogram in shared memory and adds it once. The bins are copied to
//!   a mappable buffer and read without stalling: [`ScopeEngine::take`] polls.
//! - **Reference.** [`cpu`] bins the same samples the same way; the GPU matches it within
//!   the rounding of the curve at bin edges (`tests.rs`).
//!
//! [`raster`] turns bins into the pictures the panel shows, at the panel's size.

pub mod raster;
#[cfg(test)]
pub(crate) mod tests;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::graph::Precision;

/// Levels of every scope's value axis.
pub const LEVELS: usize = 256;
/// Columns of the waveform (the picture's width in this many strips).
pub const COLS: usize = 512;
/// Side of the vectorscope's square of bins.
pub const VEC: usize = 256;
/// OkLab a and b from −range to +range across the vectorscope.
pub const VEC_RANGE: f32 = 0.4;
/// About this many pixels are counted.
pub const SAMPLES: u64 = 1 << 20;

/// Where each scope's bins sit in the buffer (in `u32` words).
pub struct Layout;

impl Layout {
    /// Histogram: R, G, B, luma; [`LEVELS`] each.
    pub const HIST: usize = 0;
    /// Waveform: luma, R, G, B planes of [`COLS`] × [`LEVELS`] (column-major: a column's
    /// levels are together).
    pub const WAVE: usize = 4 * LEVELS;
    /// Vectorscope: [`VEC`] rows (b, top = +b) of [`VEC`] (a, left = −a).
    pub const VECTOR: usize = Self::WAVE + 4 * COLS * LEVELS;
    pub const WORDS: usize = Self::VECTOR + VEC * VEC;
}

/// The step between counted pixels in both directions: about [`SAMPLES`] are counted.
pub fn stride(w: u32, h: u32) -> u32 {
    let n = u64::from(w) * u64::from(h);
    if n <= SAMPLES {
        return 1;
    }
    ((n as f64 / SAMPLES as f64).sqrt().ceil() as u32).max(1)
}

/// The counted grid's size for a `w`×`h` picture.
pub fn grid(w: u32, h: u32) -> (u32, u32) {
    let s = stride(w, h);
    (w.div_ceil(s), h.div_ceil(s))
}

/// Every scope's counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Bins {
    pub words: Vec<u32>,
    /// Pixels counted.
    pub samples: u32,
    /// The counted grid (columns × rows).
    pub grid: (u32, u32),
}

impl Bins {
    pub fn empty() -> Bins {
        Bins {
            words: vec![0; Layout::WORDS],
            samples: 0,
            grid: (0, 0),
        }
    }

    /// Histogram of channel `c` (0 R, 1 G, 2 B, 3 luma).
    pub fn hist(&self, c: usize) -> &[u32] {
        let at = Layout::HIST + c * LEVELS;
        &self.words[at..at + LEVELS]
    }

    /// Waveform plane `p` (0 luma, 1 R, 2 G, 3 B): column `x`'s levels at `x * LEVELS`.
    pub fn wave(&self, p: usize) -> &[u32] {
        let at = Layout::WAVE + p * COLS * LEVELS;
        &self.words[at..at + COLS * LEVELS]
    }

    /// The vectorscope's bins, row 0 at +b.
    pub fn vector(&self) -> &[u32] {
        &self.words[Layout::VECTOR..Layout::VECTOR + VEC * VEC]
    }
}

/// The sRGB curve (display encoding).
pub fn encode(v: f32) -> f32 {
    let l = v.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// The level bin of encoded value `e`.
pub fn level(e: f32) -> usize {
    ((e * LEVELS as f32) as i32).clamp(0, LEVELS as i32 - 1) as usize
}

/// Linear Rec.709 → LMS (Ottosson's M1 after the Rec.709 primaries, D65), row-major.
pub fn rec709_to_lms() -> [f32; 9] {
    use crate::colour::{D65_XY, REC709_PRIMARIES, mul, oklab::M1, rgb_to_xyz};
    let m = mul(&M1, &rgb_to_xyz(&REC709_PRIMARIES, D65_XY));
    std::array::from_fn(|i| m[i / 3][i % 3] as f32)
}

/// Rows 1 and 2 of OkLab's M2 (cube-rooted LMS → a, b).
pub fn m2_ab() -> [f32; 6] {
    let m = crate::colour::oklab::M2;
    [
        m[1][0] as f32,
        m[1][1] as f32,
        m[1][2] as f32,
        m[2][0] as f32,
        m[2][1] as f32,
        m[2][2] as f32,
    ]
}

/// OkLab a, b of linear Rec.709 `rgb`.
pub fn ab(rgb: [f32; 3], lms: &[f32; 9], m2: &[f32; 6]) -> [f32; 2] {
    let c = |v: f32| v.signum() * v.abs().powf(1.0 / 3.0);
    let l = [
        c(lms[0] * rgb[0] + lms[1] * rgb[1] + lms[2] * rgb[2]),
        c(lms[3] * rgb[0] + lms[4] * rgb[1] + lms[5] * rgb[2]),
        c(lms[6] * rgb[0] + lms[7] * rgb[1] + lms[8] * rgb[2]),
    ];
    [
        m2[0] * l[0] + m2[1] * l[1] + m2[2] * l[2],
        m2[3] * l[0] + m2[4] * l[1] + m2[5] * l[2],
    ]
}

/// The vectorscope bin (column from −a, row from +b) of `ab`.
pub fn vector_bin(ab: [f32; 2]) -> (usize, usize) {
    let n = VEC as f32;
    let at = |v: f32| {
        ((v / VEC_RANGE * 0.5 + 0.5) * n)
            .floor()
            .clamp(0.0, n - 1.0) as usize
    };
    (at(ab[0]), VEC - 1 - at(ab[1]))
}

/// Bins `rgba` (a `w`×`h` picture, display-linear Rec.709, values up to `peak`) on the CPU
/// exactly as the GPU pass does: the reference.
pub fn cpu(rgba: &[f32], w: u32, h: u32, peak: f32) -> Bins {
    let s = stride(w, h);
    let (gw, gh) = grid(w, h);
    let mut b = Bins::empty();
    let lms = rec709_to_lms();
    let m2 = m2_ab();
    let inv = 1.0 / peak.max(1e-6);
    for gy in 0..gh {
        for gx in 0..gw {
            let (x, y) = (gx * s, gy * s);
            let i = (y as usize * w as usize + x as usize) * 4;
            let rgb = [rgba[i], rgba[i + 1], rgba[i + 2]];
            let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
            let lv = [
                level(encode(rgb[0] * inv)),
                level(encode(rgb[1] * inv)),
                level(encode(rgb[2] * inv)),
                level(encode(luma * inv)),
            ];
            let col = (x as usize * COLS / w as usize).min(COLS - 1);
            for (c, l) in lv.into_iter().enumerate() {
                b.words[Layout::HIST + c * LEVELS + l] += 1;
            }
            // Waveform planes: luma first.
            for (p, l) in [lv[3], lv[0], lv[1], lv[2]].into_iter().enumerate() {
                b.words[Layout::WAVE + p * COLS * LEVELS + col * LEVELS + l] += 1;
            }
            let (vx, vy) = vector_bin(ab(rgb, &lms, &m2));
            b.words[Layout::VECTOR + vy * VEC + vx] += 1;
        }
    }
    b.samples = gw * gh;
    b.grid = (gw, gh);
    b
}

/// The hue (radians, from +a towards +b) of the skin-tone line: the ColorChecker's light
/// skin patch (sRGB 194, 150, 130, as published by X-Rite), whose hue most skin tones share.
pub fn skin_hue() -> f32 {
    let lin = [194.0f32, 150.0, 130.0].map(|v| {
        let e = v / 255.0;
        if e <= 0.040_45 {
            e / 12.92
        } else {
            ((e + 0.055) / 1.055).powf(2.4)
        }
    });
    let [a, b] = ab(lin, &rec709_to_lms(), &m2_ab());
    b.atan2(a)
}

/// The vectorscope's marks: OkLab a/b of a gamut's red, yellow, green, cyan, blue and
/// magenta at full strength.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Targets {
    pub ab: [[f32; 2]; 6],
}

/// The marks of linear Rec.709 (the screen's output space).
pub fn targets() -> Targets {
    let c = [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
    ];
    let (lms, m2) = (rec709_to_lms(), m2_ab());
    Targets {
        ab: c.map(|rgb| ab(rgb, &lms, &m2)),
    }
}

const SCOPE_WGSL: &str = "
struct U { w: u32, h: u32, stride: u32, prec: u32, inv_peak: f32, pad0: u32, pad1: u32, pad2: u32,
    lms0: vec4<f32>, lms1: vec4<f32>, lms2: vec4<f32>, m2a: vec4<f32>, m2b: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read_write> bins: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read> src: array<u32>;
const LEVELS: u32 = 256u;
const COLS: u32 = 512u;
const VEC: u32 = 256u;
const VEC_RANGE: f32 = 0.4;
const WAVE: u32 = 1024u;
const VECTOR: u32 = 1024u + 4u * 512u * 256u;
var<workgroup> lh: array<atomic<u32>, 1024>;
// A small cache of waveform and vectorscope bins per workgroup (slots 0..WC and WC..1024):
// neighbouring samples mostly land in a few bins, and counting those in workgroup memory
// spares the contended global atomics. A bin whose slot holds another goes straight to
// the buffer; either way every sample counts once.
const EMPTY: u32 = 0xffffffffu;
const WC: u32 = 768u;
var<workgroup> ck: array<atomic<u32>, 1024>;
var<workgroup> cn: array<atomic<u32>, 1024>;
fn put(bin: u32, base: u32, size: u32) {
    let slot = base + ((bin * 2654435761u) >> 12u) % size;
    let r = atomicCompareExchangeWeak(&ck[slot], EMPTY, bin);
    if (r.exchanged || r.old_value == bin) {
        atomicAdd(&cn[slot], 1u);
    } else {
        atomicAdd(&bins[bin], 1u);
    }
}
// Rows of samples per thread (a workgroup counts 16 × 16·ROWS samples).
const ROWS: u32 = 4u;
fn load(i: u32) -> vec4<f32> {
    if (u.prec == 1u) {
        return vec4<f32>(unpack2x16float(src[2u * i]), unpack2x16float(src[2u * i + 1u]));
    }
    return vec4<f32>(bitcast<f32>(src[4u * i]), bitcast<f32>(src[4u * i + 1u]),
        bitcast<f32>(src[4u * i + 2u]), bitcast<f32>(src[4u * i + 3u]));
}
fn enc(v: f32) -> f32 {
    let l = clamp(v, 0.0, 1.0);
    if (l <= 0.0031308) { return l * 12.92; }
    return 1.055 * pow(l, 1.0 / 2.4) - 0.055;
}
fn lvl(e: f32) -> u32 {
    return u32(clamp(i32(e * f32(LEVELS)), 0, i32(LEVELS) - 1));
}
fn cbrt(v: f32) -> f32 {
    return sign(v) * pow(abs(v), 1.0 / 3.0);
}
@compute @workgroup_size(16, 16, 1)
fn scopes(@builtin(global_invocation_id) gid: vec3<u32>,
          @builtin(workgroup_id) wid: vec3<u32>,
          @builtin(local_invocation_id) lid: vec3<u32>,
          @builtin(local_invocation_index) li: u32) {
    for (var k = 0u; k < 4u; k++) {
        atomicStore(&lh[li * 4u + k], 0u);
        atomicStore(&ck[li * 4u + k], EMPTY);
        atomicStore(&cn[li * 4u + k], 0u);
    }
    workgroupBarrier();
    let x = gid.x * u.stride;
    for (var r = 0u; r < ROWS; r++) {
    let y = (wid.y * 16u * ROWS + r * 16u + lid.y) * u.stride;
    if (x < u.w && y < u.h) {
        let rgb = load(y * u.w + x).rgb;
        let luma = 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
        let lr = lvl(enc(rgb.x * u.inv_peak));
        let lg = lvl(enc(rgb.y * u.inv_peak));
        let lb = lvl(enc(rgb.z * u.inv_peak));
        let ly = lvl(enc(luma * u.inv_peak));
        atomicAdd(&lh[lr], 1u);
        atomicAdd(&lh[LEVELS + lg], 1u);
        atomicAdd(&lh[2u * LEVELS + lb], 1u);
        atomicAdd(&lh[3u * LEVELS + ly], 1u);
        let col = min(x * COLS / u.w, COLS - 1u);
        let base = WAVE + col * LEVELS;
        put(base + ly, 0u, WC);
        put(base + COLS * LEVELS + lr, 0u, WC);
        put(base + 2u * COLS * LEVELS + lg, 0u, WC);
        put(base + 3u * COLS * LEVELS + lb, 0u, WC);
        let l0 = cbrt(dot(u.lms0.xyz, rgb));
        let l1 = cbrt(dot(u.lms1.xyz, rgb));
        let l2 = cbrt(dot(u.lms2.xyz, rgb));
        let lms = vec3<f32>(l0, l1, l2);
        let a = dot(u.m2a.xyz, lms);
        let b = dot(u.m2b.xyz, lms);
        let n = f32(VEC);
        let vx = u32(clamp(floor((a / VEC_RANGE * 0.5 + 0.5) * n), 0.0, n - 1.0));
        let vy = VEC - 1u - u32(clamp(floor((b / VEC_RANGE * 0.5 + 0.5) * n), 0.0, n - 1.0));
        put(VECTOR + vy * VEC + vx, WC, 1024u - WC);
    }
    }
    workgroupBarrier();
    for (var k = 0u; k < 4u; k++) {
        let c = atomicLoad(&lh[li * 4u + k]);
        if (c != 0u) { atomicAdd(&bins[li * 4u + k], c); }
        let key = atomicLoad(&ck[li * 4u + k]);
        let n = atomicLoad(&cn[li * 4u + k]);
        if (key != EMPTY && n != 0u) { atomicAdd(&bins[key], n); }
    }
}
";

/// Rows of samples each thread of the pass counts (`ROWS` in `SCOPE_WGSL`).
const ROWS: u32 = 4;

/// The uniform block (see `SCOPE_WGSL`'s `U`).
fn uniforms(w: u32, h: u32, prec: Precision, peak: f32) -> [u32; 28] {
    let mut u = [0u32; 28];
    u[0] = w;
    u[1] = h;
    u[2] = stride(w, h);
    u[3] = u32::from(prec == Precision::F16);
    u[4] = (1.0 / peak.max(1e-6)).to_bits();
    let lms = rec709_to_lms();
    let m2 = m2_ab();
    for r in 0..3 {
        for c in 0..3 {
            u[8 + r * 4 + c] = lms[r * 3 + c].to_bits();
        }
    }
    for r in 0..2 {
        for c in 0..3 {
            u[20 + r * 4 + c] = m2[r * 3 + c].to_bits();
        }
    }
    u
}

/// A readback buffer and its state: 0 free, 1 mapping, 2 mapped (ready), 3 failed.
struct Slot {
    buf: wgpu::Buffer,
    state: Arc<AtomicU8>,
    grid: (u32, u32),
}

/// The scopes pass on one device: its pipeline, the bins and two readback buffers (one can
/// be read while the next fills).
pub struct ScopeEngine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    bins: wgpu::Buffer,
    uniform: wgpu::Buffer,
    slots: [Slot; 2],
    next: usize,
    /// The newest pass's slot (taken by [`ScopeEngine::take`]).
    newest: Option<usize>,
    /// A pass was skipped because both readbacks were busy: render again to catch up.
    pub stale: bool,
    /// Pipelines made (one, at creation).
    pub pipelines: u64,
    /// Time the pass on the GPU (timing only: [`ScopeEngine::wait`] reads it).
    pub profile: bool,
    /// The last profiled pass's GPU time (ms).
    pub gpu_ms: Option<f64>,
    timer: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)>,
}

impl ScopeEngine {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> ScopeEngine {
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
            label: Some("develop scopes"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop scopes"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop scopes"),
            source: wgpu::ShaderSource::Wgsl(SCOPE_WGSL.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("develop scopes"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("scopes"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bytes = (Layout::WORDS * 4) as u64;
        let bins = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop scope bins"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop scope params"),
            size: 28 * 4,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let slot = || Slot {
            buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop scope read"),
                size: bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            state: Arc::new(AtomicU8::new(0)),
            grid: (0, 0),
        };
        ScopeEngine {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipeline,
            bins,
            uniform,
            slots: [slot(), slot()],
            next: 0,
            newest: None,
            stale: false,
            pipelines: 1,
            profile: false,
            gpu_ms: None,
            timer: None,
        }
    }

    /// Bins the `w`×`h` picture in `src` (packed RGBA at `prec`, values up to `peak`) on the
    /// GPU and starts reading the bins back; [`ScopeEngine::take`] has them once read.
    pub fn run(&mut self, src: &wgpu::Buffer, w: u32, h: u32, prec: Precision, peak: f32) {
        let free = [self.next, 1 - self.next]
            .into_iter()
            .find(|&i| self.slots[i].state.load(Ordering::Acquire) == 0);
        let Some(i) = free else {
            // Both still on their way: the next render catches up.
            self.stale = true;
            return;
        };
        self.stale = false;
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&uniforms(w, h, prec, peak)),
        );
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("develop scopes"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.bins.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: src.as_entire_binding(),
                },
            ],
        });
        let (gw, gh) = grid(w, h);
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("develop scopes"),
            });
        enc.clear_buffer(&self.bins, 0, None);
        let timed = self.profile
            && self
                .device
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY);
        if timed && self.timer.is_none() {
            let qs = self.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("develop scopes timer"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            });
            let buf = |usage| {
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("develop scopes timer"),
                    size: 16,
                    usage,
                    mapped_at_creation: false,
                })
            };
            self.timer = Some((
                qs,
                buf(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
                buf(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST),
            ));
        }
        let timer = self.timer.as_ref().filter(|_| timed);
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("develop scopes"),
                timestamp_writes: timer.map(|t| wgpu::ComputePassTimestampWrites {
                    query_set: &t.0,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            cp.set_pipeline(&self.pipeline);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(gw.div_ceil(16), gh.div_ceil(16 * ROWS), 1);
        }
        if let Some(t) = timer {
            enc.resolve_query_set(&t.0, 0..2, &t.1, 0);
            enc.copy_buffer_to_buffer(&t.1, 0, &t.2, 0, 16);
        }
        let slot = &mut self.slots[i];
        enc.copy_buffer_to_buffer(&self.bins, 0, &slot.buf, 0, (Layout::WORDS * 4) as u64);
        self.queue.submit([enc.finish()]);
        slot.grid = (gw, gh);
        slot.state.store(1, Ordering::Release);
        let st = slot.state.clone();
        slot.buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            st.store(if r.is_ok() { 2 } else { 3 }, Ordering::Release);
        });
        self.newest = Some(i);
        self.next = 1 - i;
    }

    /// Whether a pass is on its way.
    pub fn pending(&self) -> bool {
        self.newest
            .is_some_and(|i| self.slots[i].state.load(Ordering::Acquire) == 1)
    }

    /// The newest bins once read back (polls the device; never waits). Older passes that
    /// finished are released.
    pub fn take(&mut self) -> Option<Bins> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let mut out = None;
        for i in 0..2 {
            let st = self.slots[i].state.load(Ordering::Acquire);
            if st < 2 {
                continue;
            }
            let newest = self.newest == Some(i);
            if st == 2 && newest {
                let s = &self.slots[i];
                if let Ok(view) = s.buf.slice(..).get_mapped_range() {
                    let words: Vec<u32> = bytemuck::cast_slice(&view).to_vec();
                    out = Some(Bins {
                        samples: s.grid.0 * s.grid.1,
                        grid: s.grid,
                        words,
                    });
                }
            }
            if st == 2 {
                self.slots[i].buf.unmap();
            }
            self.slots[i].state.store(0, Ordering::Release);
            if newest {
                self.newest = None;
            }
        }
        out
    }

    /// Waits for the newest pass and returns its bins (tests, timing).
    pub fn wait(&mut self) -> Option<Bins> {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        if self.profile
            && let Some((_, _, read)) = &self.timer
        {
            read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
            if let Ok(v) = read.slice(..).get_mapped_range() {
                let t: Vec<u64> = bytemuck::pod_collect_to_vec(&v[..]);
                let period = f64::from(self.queue.get_timestamp_period()) * 1e-6;
                self.gpu_ms = Some(t[1].saturating_sub(t[0]) as f64 * period);
            }
            read.unmap();
        }
        self.take()
    }
}
