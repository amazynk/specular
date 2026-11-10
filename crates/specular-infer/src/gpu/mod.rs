//! The GPU engine: a [`Graph`] compiled once to WGSL compute dispatches, run many times.
//!
//! - Every kernel is generated from a small Rust prelude (the parameter words, the output
//!   and one loader per input, `ldK(i) -> f32`, plus `ldKv(i) -> vec4<f32>` for vec4
//!   bindings) and a WGSL body from `src/shaders`. f16 storage is read as `f16` with
//!   `SHADER_F16`, else with `unpack2x16float` from packed words; all arithmetic is f32 (f16
//!   arithmetic in the GEMM was tried: errors grew from ~2e-7 to ~1e-3 on ViT for no
//!   measurable gain). Buffers are whole vec4s long.
//! - Buffers are planned once: a value lives from its node to its last reader and then
//!   hands its buffer to later values (best fit, growing a free buffer before making a new
//!   one). Reshape is free (it shares its input's buffer). Inputs, constants and outputs
//!   have their own buffers.
//! - All parameters sit in one uniform buffer, 256 bytes per dispatch; bind groups are
//!   built once (image preprocessing rebuilds its own per run, the image size can change).
//! - A run is one compute pass per submission, in submissions of at most [`SUBMIT_MS`]
//!   estimated GPU time (`split.rs`), then one readback of every output.
//!
//! Matrix products and convolutions share one tiled GEMM (64×64 tiles, 4×4 outputs per
//! thread, f32 accumulation) specialised by injected load/store functions: strided (batched
//! matmul, 1×1 convolution, both attention products) or implicit im2col (other
//! convolutions). When an operand's contiguous index and other strides are multiples of 4
//! it is bound as `array<vec4>` and gemm_wide.wgsl loads it a vec4 at a time (1.2-1.7×
//! faster on a Radeon 860M); otherwise gemm.wgsl. Depthwise convolution has a direct
//! kernel. Row reductions (layer norm, softmax, l2 norm) reduce within subgroups first when
//! the device has `SUBGROUP`, through `subgroup_id`/`num_subgroups`, so nothing depends on
//! how the device groups invocations into subgroups or how large they are.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::cpu::{broadcast_strides, preprocess_params, resize_scale};
use crate::graph::{Def, Dft, Graph, Op, PoolKind, Preprocess, ReduceOp, ResizeMode, Value};
use crate::tensor::{DType, Data, Tensor, numel, strides};
use crate::{Error, Feed};

mod split;

pub use split::SUBMIT_MS;

const COMMON: &str = include_str!("../shaders/common.wgsl");
const ROWS: &str = include_str!("../shaders/rows.wgsl");
const ROWS_SG: &str = include_str!("../shaders/rows_sg.wgsl");
const UNARY: &str = include_str!("../shaders/unary.wgsl");
const BINARY: &str = include_str!("../shaders/binary.wgsl");
const COPY: &str = include_str!("../shaders/copy.wgsl");
const GEMM: &str = include_str!("../shaders/gemm.wgsl");
const GEMM_WIDE: &str = include_str!("../shaders/gemm_wide.wgsl");
const CONV_DIRECT: &str = include_str!("../shaders/conv_direct.wgsl");
const LAYERNORM: &str = include_str!("../shaders/layernorm.wgsl");
const L2NORM: &str = include_str!("../shaders/l2norm.wgsl");
const SOFTMAX: &str = include_str!("../shaders/softmax.wgsl");
const REDUCE: &str = include_str!("../shaders/reduce.wgsl");
const POOL: &str = include_str!("../shaders/pool.wgsl");
const RESIZE: &str = include_str!("../shaders/resize.wgsl");
const GATHER: &str = include_str!("../shaders/gather.wgsl");
const CAST_F16: &str = include_str!("../shaders/cast_f16.wgsl");
const PREPROCESS: &str = include_str!("../shaders/preprocess.wgsl");
const LSTM_GEMM: &str = include_str!("../shaders/lstm_gemm.wgsl");
const TRANSPOSE_2D: &str = include_str!("../shaders/transpose2d.wgsl");
const CHANNEL_NORM: &str = include_str!("../shaders/channel_norm.wgsl");
const GATE: &str = include_str!("../shaders/gate.wgsl");
const REDUCE_ROWS: &str = include_str!("../shaders/reduce_rows.wgsl");
const GROUP_NORM: &str = include_str!("../shaders/group_norm.wgsl");
const CONV_TRANSPOSE: &str = include_str!("../shaders/conv_transpose.wgsl");
const REFLECT_PAD: &str = include_str!("../shaders/reflect_pad.wgsl");
const DFT: &str = include_str!("../shaders/dft.wgsl");
const ZERO: &str = "@compute @workgroup_size(256)\n\
    fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,\n\
    @builtin(local_invocation_index) li: u32) {\n\
    let i = gidx(wid, nwg, li);\n\
    if (i < pu(0u)) { dst[i] = 0.0; }\n}\n";
/// The LSTM step (`lstm_gemm.wgsl`, shaped as a matrix product): sequences and states per
/// workgroup at most, threads per workgroup (twice that when sliced), and the chunk of the
/// recurrent sum loaded to workgroup memory at a time. Against the earlier kernel (each
/// weight read from memory for 2 sequences, sums in 8 slices), Tesseract's Hebrew network on
/// 16 lines at a Radeon 860M, GPU time: 41 → 32 ms (96 columns), 107 → 88 ms (256).
const LSTM_GEMM_SEQS: usize = 16;
const LSTM_GEMM_UNITS: usize = 64;
const LSTM_GEMM_THREADS: usize = 128;
const LSTM_GEMM_K: usize = 32;
/// Batches of fewer sequences split each sum into this many slices.
const LSTM_GEMM_SLICES: usize = 4;
const LSTM_GEMM_SLICED_BELOW: usize = 32;

/// Parameter words per dispatch (one 256-byte uniform slot).
const PARAM_WORDS: usize = 64;
/// Reductions over at least this many contiguous values run a workgroup per output.
const ROW_REDUCE_MIN: usize = 1024;
const PARAM_BYTES: u64 = 256;
/// Threads of a 1-D workgroup.
const WG: u64 = 256;

/// How a kernel binds one input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Bind {
    F32,
    /// f16 pairs packed in u32 words.
    F16,
    /// f32 read four at a time (`ldKv(i) -> vec4<f32>`, elements 4i..4i+4) as well.
    F32x4,
    /// f16 read four at a time as well.
    F16x4,
    /// Raw u32 words (RGBA8 pixels).
    Words,
    /// An `Rgba8Unorm` texture.
    Texture,
}

impl Bind {
    fn of(d: DType) -> Bind {
        match d {
            DType::F32 => Bind::F32,
            DType::F16 => Bind::F16,
        }
    }

    /// The same storage read four values at a time.
    fn vec4(self) -> Bind {
        match self {
            Bind::F32 => Bind::F32x4,
            Bind::F16 => Bind::F16x4,
            b => b,
        }
    }
}

/// The generated head of a kernel: parameters, output, inputs and their loaders. With
/// `in_place`, `ld0` reads the output itself (one thread reads and writes its own words).
fn prelude(out_u32: bool, ins: &[Bind], in_place: bool, f16: bool) -> String {
    let mut s = String::new();
    if f16 && (ins.contains(&Bind::F16) || ins.contains(&Bind::F16x4)) {
        s.push_str("enable f16;\n");
    }
    s.push_str(
        "struct Params { w: array<vec4<u32>, 16> }\n\
         @group(0) @binding(0) var<uniform> P: Params;\n\
         fn pu(i: u32) -> u32 { return P.w[i >> 2u][i & 3u]; }\n\
         fn pf(i: u32) -> f32 { return bitcast<f32>(pu(i)); }\n",
    );
    let ty = if out_u32 { "u32" } else { "f32" };
    let _ = writeln!(
        s,
        "@group(0) @binding(1) var<storage, read_write> dst: array<{ty}>;"
    );
    if in_place {
        s.push_str("fn ld0(i: u32) -> f32 { return dst[i]; }\n");
    }
    for (k, b) in ins.iter().enumerate() {
        let n = k + 2;
        let _ = match b {
            Bind::F32 => writeln!(
                s,
                "@group(0) @binding({n}) var<storage, read> in{k}: array<f32>;\n\
                 fn ld{k}(i: u32) -> f32 {{ return in{k}[i]; }}"
            ),
            Bind::F16 if f16 => writeln!(
                s,
                "@group(0) @binding({n}) var<storage, read> in{k}: array<f16>;\n\
                 fn ld{k}(i: u32) -> f32 {{ return f32(in{k}[i]); }}"
            ),
            Bind::F16 => writeln!(
                s,
                "@group(0) @binding({n}) var<storage, read> in{k}: array<u32>;\n\
                 fn ld{k}(i: u32) -> f32 {{ return unpack2x16float(in{k}[i >> 1u])[i & 1u]; }}"
            ),
            Bind::F32x4 => writeln!(
                s,
                "@group(0) @binding({n}) var<storage, read> in{k}: array<vec4<f32>>;\n\
                 fn ld{k}(i: u32) -> f32 {{ return in{k}[i >> 2u][i & 3u]; }}\n\
                 fn ld{k}v(i: u32) -> vec4<f32> {{ return in{k}[i]; }}"
            ),
            Bind::F16x4 if f16 => writeln!(
                s,
                "@group(0) @binding({n}) var<storage, read> in{k}: array<vec4<f16>>;\n\
                 fn ld{k}(i: u32) -> f32 {{ return f32(in{k}[i >> 2u][i & 3u]); }}\n\
                 fn ld{k}v(i: u32) -> vec4<f32> {{ return vec4<f32>(in{k}[i]); }}"
            ),
            Bind::F16x4 => writeln!(
                s,
                "@group(0) @binding({n}) var<storage, read> in{k}: array<vec2<u32>>;\n\
                 fn ld{k}(i: u32) -> f32 {{ return unpack2x16float(in{k}[i >> 2u][(i >> 1u) & 1u])[i & 1u]; }}\n\
                 fn ld{k}v(i: u32) -> vec4<f32> {{\n\
                 \x20   let w = in{k}[i];\n\
                 \x20   return vec4<f32>(unpack2x16float(w.x), unpack2x16float(w.y));\n}}"
            ),
            Bind::Words => writeln!(
                s,
                "@group(0) @binding({n}) var<storage, read> in{k}: array<u32>;"
            ),
            Bind::Texture => writeln!(s, "@group(0) @binding({n}) var in{k}: texture_2d<f32>;"),
        };
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn source(
    f16: bool,
    out_u32: bool,
    ins: &[Bind],
    in_place: bool,
    defs: &str,
    rows: bool,
    body: &str,
) -> String {
    let mut s = prelude(out_u32, ins, in_place, f16);
    s.push_str(defs);
    s.push('\n');
    s.push_str(COMMON);
    if rows {
        s.push_str(ROWS);
    }
    s.push_str(body);
    s
}

/// Outputs per GEMM workgroup along M and N (see gemm.wgsl). Smaller tiles for small
/// products were measured slower on an RDNA iGPU, even at a few dozen workgroups.
const TILE: u64 = 64;

/// The tile-load order constants of gemm.wgsl.
fn gemm_order(a_kmajor: bool, b_nmajor: bool) -> String {
    format!("const A_KMAJOR: bool = {a_kmajor};\nconst B_NMAJOR: bool = {b_nmajor};\n")
}

/// gemm_wide.wgsl with its constants: 64-row tiles, 4 × 4 outputs per thread (as gemm.wgsl;
/// 8 × 4 per thread and 128-row tiles were not faster on a Radeon 860M), vec4 loads of A
/// and/or B.
fn wide_source(f16: bool, kinds: &[Bind], defs: &str, a_vec: bool, b_vec: bool) -> String {
    let defs = format!(
        "{defs}const TM: u32 = {TILE}u;\nconst RM: u32 = 4u;\nconst A_VEC: bool = {a_vec};\n\
         const B_VEC: bool = {b_vec};\n"
    );
    source(f16, false, kinds, false, &defs, false, GEMM_WIDE)
}

/// The GEMM epilogue `store_c` writing `dst[i]` (the expression `index`): bias (the
/// expression `bias`, if any), activation code `pu(20u)`, and the residual `ld{res}(i)`.
fn store_c(index: &str, bias: Option<&str>, res: Option<usize>) -> String {
    let bias = bias.map_or(String::new(), |b| format!(" + {b}"));
    let res = res.map_or(String::new(), |r| format!(" + ld{r}(i)"));
    format!(
        "fn store_c(z1: u32, z2: u32, m: u32, n: u32, v: f32) {{\n\
         \x20   let i = {index};\n\
         \x20   dst[i] = act(v{bias}, pu(20u)){res};\n}}\n"
    )
}

/// Operand access of the strided GEMM. Parameters: 0 M, 1 N, 2 K, 3 inner batch; A at 4
/// (base, batch1, batch2, m, k), B at 9 (base, batch1, batch2, k, n), C at 14 (base,
/// batch1, batch2, m, n), bias strides batch2 @19, m @21, n @22; act @20. The bias is
/// `ld2`, the residual (laid out as C) `ld{res}`.
fn strided_defs(bias: bool, res: Option<usize>, (a_vec, b_vec): (bool, bool)) -> String {
    let mut s = String::from(
        "fn load_a(z1: u32, z2: u32, m: u32, k: u32) -> f32 {\n\
         \x20   return ld0(pu(4u) + z1 * pu(5u) + z2 * pu(6u) + m * pu(7u) + k * pu(8u));\n}\n\
         fn load_b(z1: u32, z2: u32, k: u32, n: u32) -> f32 {\n\
         \x20   return ld1(pu(9u) + z1 * pu(10u) + z2 * pu(11u) + k * pu(12u) + n * pu(13u));\n}\n",
    );
    s.push_str(if a_vec {
        "fn load_a4(z1: u32, z2: u32, m: u32, k: u32) -> vec4<f32> {\n\
         \x20   return ld0v((pu(4u) + z1 * pu(5u) + z2 * pu(6u) + m * pu(7u) + k) >> 2u);\n}\n"
    } else {
        NO_LOAD_A4
    });
    s.push_str(if b_vec {
        "fn load_b4(z1: u32, z2: u32, k: u32, n: u32) -> vec4<f32> {\n\
         \x20   return ld1v((pu(9u) + z1 * pu(10u) + z2 * pu(11u) + k * pu(12u) + n * pu(13u)) >> 2u);\n}\n"
    } else {
        NO_LOAD_B4
    });
    s.push_str(&store_c(
        "pu(14u) + z1 * pu(15u) + z2 * pu(16u) + m * pu(17u) + n * pu(18u)",
        bias.then_some("ld2(z2 * pu(19u) + m * pu(21u) + n * pu(22u))"),
        res,
    ));
    s
}

/// Operand access of the implicit-im2col convolution GEMM: A = the weights `in1`, B = the
/// input `in0` gathered on the fly. Parameters as `Planner::conv`.
const NO_LOAD_A4: &str =
    "fn load_a4(z1: u32, z2: u32, m: u32, k: u32) -> vec4<f32> { return vec4<f32>(0.0); }\n";
const NO_LOAD_B4: &str =
    "fn load_b4(z1: u32, z2: u32, k: u32, n: u32) -> vec4<f32> { return vec4<f32>(0.0); }\n";

fn conv_defs(bias: bool, res: Option<usize>, a_vec: bool) -> String {
    let mut s = String::from(if a_vec {
        "fn load_a4(z1: u32, z2: u32, m: u32, k: u32) -> vec4<f32> {\n\
         \x20   return ld1v(((z2 * pu(0u) + m) * pu(2u) + k) >> 2u);\n}\n"
    } else {
        NO_LOAD_A4
    });
    s.push_str(NO_LOAD_B4);
    s.push_str(
        "fn load_a(z1: u32, z2: u32, m: u32, k: u32) -> f32 {\n\
         \x20   return ld1((z2 * pu(0u) + m) * pu(2u) + k);\n}\n\
         fn load_b(z1: u32, z2: u32, k: u32, n: u32) -> f32 {\n\
         \x20   let kk = pu(7u) * pu(8u);\n\
         \x20   let ci = k / kk;\n\
         \x20   let r = k % kk;\n\
         \x20   let iy = i32((n / pu(9u)) * pu(10u) + (r / pu(8u)) * pu(14u)) - i32(pu(12u));\n\
         \x20   let ix = i32((n % pu(9u)) * pu(11u) + (r % pu(8u)) * pu(15u)) - i32(pu(13u));\n\
         \x20   if (iy < 0 || ix < 0 || iy >= i32(pu(5u)) || ix >= i32(pu(6u))) {\n\
         \x20       return 0.0;\n\
         \x20   }\n\
         \x20   return ld0(((z1 * pu(4u) + z2 * pu(16u) + ci) * pu(5u) + u32(iy)) * pu(6u) + u32(ix));\n}\n",
    );
    s.push_str(&store_c(
        "(z1 * pu(18u) + z2 * pu(0u) + m) * pu(1u) + n",
        bias.then_some("ld2(z2 * pu(0u) + m)"),
        res,
    ));
    s
}

/// The workgroup-size constants of the row kernels: `RW` threads per row, `RP`, the power
/// of two at or above it (for rows.wgsl's tree reduction), and `SGS`, the most subgroups a
/// workgroup can have given the device's smallest subgroup size `sg_min` (for rows_sg.wgsl:
/// a constant bound lets the compiler unroll its loop over the subgroups' partials).
fn rows_defs(wg: u32, sg_min: u32) -> String {
    format!(
        "const RW: u32 = {wg}u;\nconst RP: u32 = {}u;\nconst SGS: u32 = {}u;\n",
        wg.next_power_of_two(),
        wg.div_ceil(sg_min.max(1))
    )
}

/// A row kernel's source (without rows.wgsl) made to reduce through subgroups: rows_sg.wgsl,
/// and the entry point told its subgroup's id, the number of subgroups and its lane.
fn rows_subgroups(src: &str) -> String {
    const ENTRY: &str = "@builtin(local_invocation_index) li: u32) {";
    debug_assert!(src.contains(ENTRY));
    let (head, body) = src.split_at(src.find("@compute").unwrap_or(src.len()));
    format!(
        "{head}{ROWS_SG}{}",
        body.replacen(
            ENTRY,
            "@builtin(local_invocation_index) li: u32,\n        @builtin(subgroup_id) sgi: u32,\n        \
             @builtin(num_subgroups) sgc: u32,\n        @builtin(subgroup_invocation_id) sgl: u32) {\n    \
             sg_id = sgi;\n    sg_count = sgc;\n    sg_lane = sgl;",
            1
        )
    )
}

fn px_defs(texture: bool) -> &'static str {
    if texture {
        "fn px(x: u32, y: u32) -> vec4<f32> { return textureLoad(in0, vec2<i32>(i32(x), i32(y)), 0); }\n"
    } else {
        "fn px(x: u32, y: u32) -> vec4<f32> { return unpack4x8unorm(in0[y * pu(2u) + x]); }\n"
    }
}

/// A compiled kernel.
#[derive(Clone)]
struct Pipeline {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

struct Inner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    limits: wgpu::Limits,
    features: Features,
    /// Threads per row of the row kernels (layer norm, softmax, l2 norm).
    rows_wg: u32,
    /// The smallest subgroup size the device may use.
    sg_min: u32,
    cache: Mutex<HashMap<String, Pipeline>>,
    /// Set once the device was found lost (a driver reset): nothing made on it works any
    /// more, and every later use returns [`Error::Lost`] at once.
    lost: AtomicBool,
    /// Learnt dispatch times, for splitting runs into short submissions.
    speeds: Mutex<split::Speeds>,
    /// The submission limit of graphs without their own (ms, as f64 bits).
    submit_ms: AtomicU64,
    /// Runs since [`Context::take_stats`].
    stats: Mutex<RunTotals>,
}

/// Optional device features the kernels use when the device has them. Every kernel has a
/// plain fallback; the results agree with the CPU reference within the same tolerances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Features {
    /// `Features::SHADER_F16`: f16 weights read as `f16` (two per load) rather than unpacked
    /// from u32 words. Arithmetic stays f32 either way.
    pub f16: bool,
    /// `Features::TIMESTAMP_QUERY`: [`GpuGraph::profile`] works.
    pub timestamps: bool,
    /// `Features::SUBGROUP`: row reductions (layer norm, softmax, l2 norm) reduce within
    /// subgroups first.
    pub subgroups: bool,
}

impl Features {
    /// Everything the device was opened with.
    pub fn of(device: &wgpu::Device) -> Features {
        let f = device.features();
        Features {
            f16: f.contains(wgpu::Features::SHADER_F16),
            timestamps: f.contains(wgpu::Features::TIMESTAMP_QUERY),
            subgroups: f.contains(wgpu::Features::SUBGROUP),
        }
    }

    /// None of them: the plain kernels.
    pub fn none() -> Features {
        Features {
            f16: false,
            timestamps: false,
            subgroups: false,
        }
    }

    /// The device features worth requesting when opening a device for inference.
    pub fn wanted() -> wgpu::Features {
        wgpu::Features::SHADER_F16 | wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::SUBGROUP
    }
}

/// A device, its queue and the kernels compiled for it (shared by every graph built on it).
#[derive(Clone)]
pub struct Context {
    inner: Arc<Inner>,
}

impl Context {
    /// A context using every optional feature the device has.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Context {
        Context::with_features(device, queue, Features::of(device))
    }

    /// A context using only `features` (and only those the device has): tests use it to
    /// run the plain kernels on a device that has the fast ones.
    pub fn with_features(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        features: Features,
    ) -> Context {
        Context::make(device, queue, features, WG as u32)
    }

    fn make(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        features: Features,
        rows_wg: u32,
    ) -> Context {
        let have = Features::of(device);
        // Never below the smallest size WGSL allows (also if the backend reports none).
        let sg_min = device
            .adapter_info()
            .subgroup_min_size
            .max(wgpu::MINIMUM_SUBGROUP_MIN_SIZE);
        Context {
            inner: Arc::new(Inner {
                device: device.clone(),
                queue: queue.clone(),
                limits: device.limits(),
                features: Features {
                    f16: features.f16 && have.f16,
                    timestamps: features.timestamps && have.timestamps,
                    subgroups: features.subgroups && have.subgroups,
                },
                rows_wg,
                sg_min,
                cache: Mutex::new(HashMap::new()),
                lost: AtomicBool::new(false),
                speeds: Mutex::new(split::Speeds::default()),
                submit_ms: AtomicU64::new(SUBMIT_MS.to_bits()),
                stats: Mutex::new(RunTotals::default()),
            }),
        }
    }

    /// As [`Context::with_features`], with `threads` (1 to 256, within the device's limit)
    /// per row in the row kernels rather than 256: tests use it to check the reductions at
    /// workgroup sizes that are not a multiple of the subgroup size.
    #[doc(hidden)]
    pub fn with_row_threads(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        features: Features,
        threads: u32,
    ) -> Context {
        let limit = device.limits().max_compute_invocations_per_workgroup;
        Context::make(
            device,
            queue,
            features,
            threads.clamp(1, (WG as u32).min(limit)),
        )
    }

    /// Threads per row of the row kernels (256 unless made by `with_row_threads`).
    #[doc(hidden)]
    pub fn row_threads(&self) -> u32 {
        self.inner.rows_wg
    }

    pub fn features(&self) -> Features {
        self.inner.features
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.inner.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.inner.queue
    }

    /// Sets the estimated GPU milliseconds one queue submission may hold for every graph on
    /// this context without its own limit ([`GpuGraph::set_submission_ms`]); default
    /// [`SUBMIT_MS`], 0 submits every dispatch on its own, infinity a whole run at once.
    pub fn set_submission_ms(&self, ms: f64) {
        self.inner
            .submit_ms
            .store(ms.max(0.0).to_bits(), Ordering::Relaxed);
    }

    pub fn submission_ms(&self) -> f64 {
        f64::from_bits(self.inner.submit_ms.load(Ordering::Relaxed))
    }

    /// The runs of every graph on this context since the last call; starts counting afresh.
    pub fn take_stats(&self) -> RunTotals {
        let mut s = self.inner.stats.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *s)
    }

    /// The learnt dispatch times.
    fn speeds(&self) -> std::sync::MutexGuard<'_, split::Speeds> {
        self.inner.speeds.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether the device was found lost (a driver reset). A lost context stays lost: make a
    /// new one on the new device.
    pub fn is_lost(&self) -> bool {
        self.inner.lost.load(Ordering::Acquire)
    }

    /// `Device::poll` that survives a lost device (wgpu panics in `poll` once the device is
    /// gone): that becomes [`Error::Lost`] and the context is marked lost.
    pub fn poll(&self, ty: wgpu::PollType) -> Result<(), Error> {
        if self.is_lost() {
            return Err(Error::Lost);
        }
        let r = poll_device(&self.inner.device, ty);
        if r == Err(Error::Lost) {
            self.inner.lost.store(true, Ordering::Release);
        }
        r
    }

    /// Whether the device no longer works: marked lost, a poll now panics, or a small
    /// buffer cannot be mapped. A destroyed device takes work and reports no errors (WebGPU
    /// says nothing on a lost device); only its maps fail.
    fn gone(&self) -> bool {
        if self.is_lost() || self.poll(wgpu::PollType::Poll) == Err(Error::Lost) {
            return true;
        }
        let device = &self.inner.device;
        let mapped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let probe = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("infer loss probe"),
                size: 4,
                usage: wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let (tx, rx) = std::sync::mpsc::channel();
            probe.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r.is_ok());
            });
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            rx.recv_timeout(std::time::Duration::from_secs(5))
        }));
        // A map that comes back failed; one not back yet (another thread collected the
        // callback) says nothing.
        let gone = matches!(mapped, Err(_) | Ok(Ok(false)));
        if gone {
            self.inner.lost.store(true, Ordering::Release);
        }
        gone
    }

    /// Runs `f`, which uses the device: [`Error::Lost`] at once on a lost context, and when
    /// `f` fails or panics in wgpu because the device went meanwhile. Other panics go on.
    pub(crate) fn guard<T>(&self, f: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        if self.is_lost() {
            return Err(Error::Lost);
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(_)) | Err(_) if self.gone() => Err(Error::Lost),
            Ok(Err(e)) => Err(e),
            Err(p) => std::panic::resume_unwind(p),
        }
    }

    fn pipeline(&self, src: &str, ins: &[Bind]) -> Result<Pipeline, Error> {
        let mut cache = self.inner.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = cache.get(src) {
            return Ok(p.clone());
        }
        let device = &self.inner.device;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("infer kernel"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let buffer =
            |binding: u32, ty: wgpu::BufferBindingType, min: u64| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(min),
                },
                count: None,
            };
        let mut entries = vec![
            buffer(0, wgpu::BufferBindingType::Uniform, PARAM_BYTES),
            buffer(1, wgpu::BufferBindingType::Storage { read_only: false }, 0),
        ];
        for (k, b) in ins.iter().enumerate() {
            let binding = k as u32 + 2;
            entries.push(match b {
                Bind::Texture => wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                _ => buffer(
                    binding,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    0,
                ),
            });
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("infer kernel"),
            entries: &entries,
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("infer kernel"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("infer kernel"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(Error::Gpu(format!("kernel does not build: {e}")));
        }
        let p = Pipeline { pipeline, layout };
        cache.insert(src.to_owned(), p.clone());
        Ok(p)
    }
}

/// What a dispatch binds after the output.
#[derive(Clone, Copy, Debug)]
enum Res {
    Slot(usize),
    /// An image input (a buffer of RGBA8 words or a texture, bound per run: the step's `image`).
    Image,
}

/// A dispatch as planned (before buffers exist).
struct StepPlan {
    src: String,
    ins: Vec<Bind>,
    res: Vec<Res>,
    dst: usize,
    groups: [u32; 3],
    params: [u32; PARAM_WORDS],
    /// Image preprocessing: the graph input it reads and its parameters (set per run).
    image: Option<(usize, Preprocess)>,
    /// What the step computes, for [`GpuGraph::profile`].
    label: String,
    /// Estimated work units (its node's, shared among the node's steps).
    work: f64,
}

struct Planner<'g> {
    g: &'g Graph,
    max_groups: u64,
    max_binding: u64,
    slots: Vec<u64>,
    free: Vec<usize>,
    vslot: Vec<Option<usize>>,
    steps: Vec<StepPlan>,
    /// Label of the node being planned.
    label: String,
    /// f16 storage read as `f16` (`Features::f16`).
    f16: bool,
    /// Row reductions through subgroups (`Features::subgroups`).
    subgroups: bool,
    /// Threads per row of the row kernels.
    rows_wg: u32,
    /// The device's smallest subgroup size.
    sg_min: u32,
    /// Per node: the residual add fused into its GEMM epilogue (the add's output and its
    /// other operand). The add itself is then skipped.
    fused: Vec<Option<(Value, Value)>>,
    skip: Vec<bool>,
}

/// Bytes of a value on the GPU (whole words, at least one).
fn value_bytes(shape: &[usize], dtype: DType) -> u64 {
    let n = numel(shape).unwrap_or(0) as u64 * dtype.size() as u64;
    n.div_ceil(4).max(1) * 4
}

fn u(x: usize) -> u32 {
    // Graph building keeps every size and index below 2³¹.
    x as u32
}

fn gpu_err<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error::Gpu(msg.into()))
}

impl Planner<'_> {
    fn alloc(&mut self, bytes: u64) -> Result<usize, Error> {
        if bytes > self.max_binding {
            return gpu_err(format!(
                "a {bytes}-byte buffer is above this device's binding limit"
            ));
        }
        let fit = self
            .free
            .iter()
            .enumerate()
            .filter(|&(_, &s)| self.slots[s] >= bytes)
            .min_by_key(|&(_, &s)| self.slots[s])
            .map(|(i, _)| i);
        let pick = fit.or_else(|| {
            self.free
                .iter()
                .enumerate()
                .max_by_key(|&(_, &s)| self.slots[s])
                .map(|(i, _)| i)
        });
        Ok(match pick {
            Some(i) => {
                let s = self.free.swap_remove(i);
                self.slots[s] = self.slots[s].max(bytes);
                s
            }
            None => {
                self.slots.push(bytes);
                self.slots.len() - 1
            }
        })
    }

    fn dedicated(&mut self, bytes: u64) -> Result<usize, Error> {
        if bytes > self.max_binding {
            return gpu_err(format!(
                "a {bytes}-byte buffer is above this device's binding limit"
            ));
        }
        self.slots.push(bytes);
        Ok(self.slots.len() - 1)
    }

    fn slot(&self, v: Value) -> usize {
        self.vslot[v.index()].expect("planned value")
    }

    fn bind(&self, v: Value) -> (Bind, Res) {
        (Bind::of(self.g.dtype(v)), Res::Slot(self.slot(v)))
    }

    /// Workgroup grid for `groups` workgroups of a 1-D or per-row kernel.
    fn grid(&self, groups: u64) -> Result<[u32; 3], Error> {
        if groups == 0 {
            return Ok([0, 0, 0]);
        }
        let x = groups.min(self.max_groups);
        let y = groups.div_ceil(x);
        if y > self.max_groups {
            return gpu_err(format!("{groups} workgroups do not fit a dispatch"));
        }
        Ok([x as u32, y as u32, 1])
    }

    fn gemm_grid(&self, m: usize, n: usize, batch: usize) -> Result<[u32; 3], Error> {
        let x = (n as u64).div_ceil(TILE);
        let y = (m as u64).div_ceil(TILE);
        let z = batch as u64;
        if x > self.max_groups || y > self.max_groups || z > self.max_groups {
            return gpu_err(format!(
                "a {m}x{n} (x{batch}) product does not fit a dispatch"
            ));
        }
        Ok([x as u32, y as u32, z as u32])
    }

    fn push(
        &mut self,
        src: String,
        ins: Vec<(Bind, Res)>,
        dst: usize,
        groups: [u32; 3],
        params: [u32; PARAM_WORDS],
    ) {
        if groups.contains(&0) {
            return;
        }
        let (ins, res) = ins.into_iter().unzip();
        self.steps.push(StepPlan {
            src,
            ins,
            res,
            dst,
            groups,
            params,
            image: None,
            label: self.label.clone(),
            work: 0.0,
        });
    }

    fn push_1d(
        &mut self,
        body: &str,
        ins: Vec<(Bind, Res)>,
        dst: usize,
        threads: usize,
        params: [u32; PARAM_WORDS],
    ) -> Result<(), Error> {
        let kinds: Vec<Bind> = ins.iter().map(|i| i.0).collect();
        let src = source(self.f16, false, &kinds, false, "", false, body);
        let groups = self.grid((threads as u64).div_ceil(WG))?;
        self.push(src, ins, dst, groups, params);
        Ok(())
    }

    fn push_rows(
        &mut self,
        body: &str,
        ins: Vec<(Bind, Res)>,
        dst: usize,
        rows: usize,
        params: [u32; PARAM_WORDS],
        in_place: bool,
    ) -> Result<(), Error> {
        let kinds: Vec<Bind> = ins.iter().map(|i| i.0).collect();
        let defs = rows_defs(self.rows_wg, self.sg_min);
        let src = if self.subgroups {
            rows_subgroups(&source(
                self.f16, false, &kinds, in_place, &defs, false, body,
            ))
        } else {
            source(self.f16, false, &kinds, in_place, &defs, true, body)
        };
        let groups = self.grid(rows as u64)?;
        self.push(src, ins, dst, groups, params);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn push_strided_gemm(
        &mut self,
        ins: Vec<(Bind, Res)>,
        dst: usize,
        (m, n, k): (usize, usize, usize),
        (b1, b2): (usize, usize),
        a: [usize; 5],
        b: [usize; 5],
        c: [usize; 5],
        bias: Option<[usize; 3]>,
        act: u32,
        res: Option<usize>,
    ) -> Result<(), Error> {
        let mut p = [0u32; PARAM_WORDS];
        p[0] = u(m);
        p[1] = u(n);
        p[2] = u(k);
        p[3] = u(b2.max(1));
        for i in 0..5 {
            p[4 + i] = u(a[i]);
            p[9 + i] = u(b[i]);
            p[14 + i] = u(c[i]);
        }
        if let Some([s2, sm, sn]) = bias {
            p[19] = u(s2);
            p[21] = u(sm);
            p[22] = u(sn);
        }
        p[20] = act;
        // Contiguous index of the tile loads: k for A when its k stride is 1, n for B when
        // its n stride is 1.
        let a_kmajor = a[4] == 1 || a[3] != 1;
        let b_nmajor = b[4] == 1 || b[3] != 1;
        // vec4 loads: along the contiguous index, when it is a multiple of 4 long and every
        // other stride and the base are multiples of 4.
        let x4 = |v: &[usize]| v.iter().all(|&x| x % 4 == 0);
        let a_vec = a[4] == 1 && k % 4 == 0 && x4(&a[..4]);
        let b_vec = if b_nmajor {
            b[4] == 1 && n % 4 == 0 && x4(&b[..4])
        } else {
            b[3] == 1 && k % 4 == 0 && x4(&[b[0], b[1], b[2], b[4]])
        };
        let mut ins = ins;
        if a_vec {
            ins[0].0 = ins[0].0.vec4();
        }
        if b_vec {
            ins[1].0 = ins[1].0.vec4();
        }
        let kinds: Vec<Bind> = ins.iter().map(|i| i.0).collect();
        let mut defs = gemm_order(a_kmajor, b_nmajor);
        defs.push_str(&strided_defs(bias.is_some(), res, (a_vec, b_vec)));
        // The wide kernel only pays off through its vec4 loads.
        let src = if a_vec || b_vec {
            wide_source(self.f16, &kinds, &defs, a_vec, b_vec)
        } else {
            source(self.f16, false, &kinds, false, &defs, false, GEMM)
        };
        let groups = self.gemm_grid(m, n, b1 * b2)?;
        self.push_gemm(src, ins, dst, groups, p, m * n * k * b1 * b2);
        Ok(())
    }

    /// Pushes a GEMM dispatch, in several along its rows of tiles (parameter 23: the first
    /// row) when it has more than [`GEMM_SPLIT_MACS`] multiply-adds: one dispatch can't be
    /// split into submissions (UperNet's 3×3 convolution over 2048 channels at 96²: 87 G
    /// multiply-adds, 459 ms on a Radeon 860M).
    fn push_gemm(
        &mut self,
        src: String,
        ins: Vec<(Bind, Res)>,
        dst: usize,
        groups: [u32; 3],
        mut p: [u32; PARAM_WORDS],
        macs: usize,
    ) {
        let rows = groups[1] as usize;
        let pieces = macs.div_ceil(GEMM_SPLIT_MACS).clamp(1, rows.max(1));
        let per = rows.div_ceil(pieces);
        let mut y = 0;
        while y < rows {
            let h = per.min(rows - y);
            p[23] = u(y);
            self.push(
                src.clone(),
                ins.clone(),
                dst,
                [groups[0], u(h), groups[2]],
                p,
            );
            y += h;
        }
    }

    fn node(&mut self, i: usize) -> Result<(), Error> {
        let g = self.g;
        let node = &g.nodes[i];
        if self.skip[i] {
            return Ok(());
        }
        let fused = self.fused[i];
        // A fused product writes the add's output; its own value is never read.
        let out_v = fused.map_or(node.output, |f| f.0);
        let out_shape = g.shape(out_v).to_vec();
        let n_out = numel(&out_shape).unwrap_or(0);
        if let Op::Reshape = node.op {
            self.vslot[out_v.index()] = self.vslot[node.inputs[0].index()];
            return Ok(());
        }
        let mut bytes = value_bytes(&out_shape, g.dtype(out_v));
        if let Op::Lstm { .. } = node.op {
            // The LSTM's cells and hidden state (twice) after its output.
            let s = g.shape(node.inputs[0])[0] * g.shape(node.inputs[1])[0];
            bytes += 3 * 4 * s as u64;
        }
        let dst = if g.outputs.contains(&out_v) {
            self.dedicated(bytes)?
        } else {
            self.alloc(bytes)?
        };
        self.vslot[out_v.index()] = Some(dst);
        let ins: Vec<(Bind, Res)> = node
            .inputs
            .iter()
            .chain(fused.as_ref().map(|f| &f.1))
            .map(|&v| {
                if matches!(g.info(v).def, Def::Image(_)) {
                    (Bind::Words, Res::Image)
                } else {
                    self.bind(v)
                }
            })
            .collect();
        let shape = |k: usize| g.shape(node.inputs[k]);
        self.label = op_label(&node.op, &out_shape);
        let mut p = [0u32; PARAM_WORDS];
        match &node.op {
            Op::Reshape => unreachable!(),
            Op::Unary { act, mul, add } => {
                p[0] = u(n_out);
                p[1] = act.code();
                p[2] = mul.to_bits();
                p[3] = add.to_bits();
                self.push_1d(UNARY, ins, dst, n_out, p)?;
            }
            Op::Binary(op) => {
                p[0] = u(n_out);
                p[1] = op.code();
                p[2] = u(out_shape.len());
                let sa = broadcast_strides(shape(0), &out_shape);
                let sb = broadcast_strides(shape(1), &out_shape);
                for d in 0..out_shape.len() {
                    p[4 + d] = u(out_shape[d]);
                    p[10 + d] = u(sa[d]);
                    p[16 + d] = u(sb[d]);
                }
                self.push_1d(BINARY, ins, dst, n_out, p)?;
            }
            Op::Transpose { perm } => {
                if let Some((b, r, c)) = batched_swap(shape(0), perm)
                    && n_out >= TRANSPOSE_2D_MIN
                {
                    p[0] = u(b);
                    p[1] = u(r);
                    p[2] = u(c);
                    let (tc, tr) = (c.div_ceil(32), r.div_ceil(32));
                    p[3] = u(tc);
                    p[4] = u(tr);
                    let kinds = [ins[0].0];
                    let src = source(self.f16, false, &kinds, false, "", false, TRANSPOSE_2D);
                    let groups = self.grid((b * tc * tr) as u64)?;
                    self.push(src, ins, dst, groups, p);
                    return Ok(());
                }
                let st = strides(shape(0));
                let src: Vec<usize> = perm.iter().map(|&q| st[q]).collect();
                self.copy(ins[0], dst, &out_shape, &src, 0, &strides(&out_shape), 0)?;
            }
            Op::Slice { starts, steps, .. } => {
                let st = strides(shape(0));
                let base = starts.iter().zip(&st).map(|(a, b)| a * b).sum();
                let src: Vec<usize> = st.iter().zip(steps).map(|(a, b)| a * b).collect();
                self.copy(ins[0], dst, &out_shape, &src, base, &strides(&out_shape), 0)?;
            }
            Op::Concat { axis } => {
                let ost = strides(&out_shape);
                let mut at = 0;
                for (k, &inp) in ins.iter().enumerate() {
                    let s = shape(k).to_vec();
                    self.copy(inp, dst, &s, &strides(&s), 0, &ost, at * ost[*axis])?;
                    at += s[*axis];
                }
            }
            Op::MatMul(mm) => {
                let (sa, sb) = (shape(0).to_vec(), shape(1).to_vec());
                let r = out_shape.len();
                let (m, n) = (out_shape[r - 2], out_shape[r - 1]);
                let k = sa[sa.len() - 1];
                let pad2 = |s: &[usize]| -> [usize; 2] {
                    let b = &s[..s.len() - 2];
                    match b.len() {
                        0 => [1, 1],
                        1 => [1, b[0]],
                        _ => [b[b.len() - 2], b[b.len() - 1]],
                    }
                };
                let [b1, b2] = pad2(&out_shape);
                let batch_strides = |d: [usize; 2], mat: usize| -> [usize; 2] {
                    [
                        if d[0] == 1 { 0 } else { d[1] * mat },
                        if d[1] == 1 { 0 } else { mat },
                    ]
                };
                let [a1, a2] = batch_strides(pad2(&sa), m * k);
                let [c1, c2] = batch_strides(pad2(&sb), k * n);
                let (bk, bn) = if mm.trans_b { (1, k) } else { (n, 1) };
                let bias = (node.inputs.len() > 2).then_some([0, 0, 1]);
                let res = fused.map(|_| ins.len() - 1);
                self.push_strided_gemm(
                    ins,
                    dst,
                    (m, n, k),
                    (b1, b2),
                    [0, a1, a2, k, 1],
                    [0, c1, c2, bk, bn],
                    [0, b2 * m * n, m * n, n, 1],
                    bias,
                    mm.act.code(),
                    res,
                )?;
            }
            Op::Conv2d(cp) => {
                let bias = node.inputs.len() > 2;
                let res = fused.map(|_| ins.len() - 1);
                let shapes = (shape(0), shape(1), &out_shape[..]);
                self.conv(cp, ins, dst, shapes, bias, res)?;
            }
            Op::Attention {
                heads,
                scale,
                causal,
                packed,
            } => {
                let s0 = shape(0).to_vec();
                let (nb, tq) = (s0[0], s0[1]);
                let d = out_shape[2];
                let (tk, rq, rk, oq, ok, ov) = if *packed {
                    (tq, 3 * d, 3 * d, 0, d, 2 * d)
                } else {
                    (shape(1)[1], d, d, 0, 0, 0)
                };
                let (q, kk, v) = if *packed {
                    (ins[0], ins[0], ins[0])
                } else {
                    (ins[0], ins[1], ins[2])
                };
                let h = *heads;
                let hd = d / h;
                let scratch = self.alloc(value_bytes(&[nb, h, tq, tk], DType::F32))?;
                // S = Q·Kᵀ per (image, head): [N, H, Tq, Tk].
                self.push_strided_gemm(
                    vec![q, kk],
                    scratch,
                    (tq, tk, hd),
                    (nb, h),
                    [oq, tq * rq, hd, rq, 1],
                    [ok, tk * rk, hd, 1, rk],
                    [0, h * tq * tk, tq * tk, tk, 1],
                    None,
                    0,
                    None,
                )?;
                let mut sp = [0u32; PARAM_WORDS];
                sp[0] = u(nb * h * tq);
                sp[1] = u(tk);
                sp[2] = scale.to_bits();
                sp[3] = u32::from(*causal);
                sp[4] = u(tq);
                self.push_rows(SOFTMAX, vec![], scratch, nb * h * tq, sp, true)?;
                // O = S·V into [N, Tq, D] at the head's columns.
                self.push_strided_gemm(
                    vec![(Bind::F32, Res::Slot(scratch)), v],
                    dst,
                    (tq, hd, tk),
                    (nb, h),
                    [0, h * tq * tk, tq * tk, tk, 1],
                    [ov, tk * rk, hd, rk, 1],
                    [0, tq * d, hd, d, 1],
                    None,
                    0,
                    None,
                )?;
                self.free.push(scratch);
            }
            Op::LayerNorm { eps } | Op::L2Norm { eps } => {
                let d = *out_shape.last().expect("rank");
                let rows = n_out / d;
                p[0] = u(rows);
                p[1] = u(d);
                p[2] = eps.to_bits();
                let body = if matches!(node.op, Op::LayerNorm { .. }) {
                    LAYERNORM
                } else {
                    L2NORM
                };
                self.push_rows(body, ins, dst, rows, p, false)?;
            }
            Op::ChannelNorm { eps } => {
                let s = shape(0);
                let hw = s[2] * s[3];
                p[0] = u(s[0] * hw);
                p[1] = u(s[1]);
                p[2] = u(hw);
                p[3] = eps.to_bits();
                self.push_1d(CHANNEL_NORM, ins, dst, s[0] * hw, p)?;
            }
            Op::Gate => {
                let s = shape(0);
                let half = s[1] / 2 * s[2] * s[3];
                p[0] = u(n_out);
                p[1] = u(half);
                self.push_1d(GATE, ins, dst, n_out, p)?;
            }
            Op::DepthToSpace { block } => {
                let (dims, src) = crate::cpu::depth_to_space_strides(shape(0), *block);
                self.copy(ins[0], dst, &dims, &src, 0, &strides(&dims), 0)?;
            }
            Op::GroupNorm { groups, eps } => {
                let s = shape(0);
                let hw = s[2] * s[3];
                let rows = s[0] * groups;
                p[0] = u(rows);
                p[1] = u(s[1] / groups * hw);
                p[2] = eps.to_bits();
                p[3] = u(hw);
                p[4] = u(s[1] / groups);
                p[5] = u(*groups);
                self.push_rows(GROUP_NORM, ins, dst, rows, p, false)?;
            }
            Op::ConvTranspose2d { stride, pad, act } => {
                let (s, w) = (shape(0).to_vec(), shape(1).to_vec());
                let vals = [
                    n_out,
                    s[1],
                    s[2],
                    s[3],
                    w[1],
                    w[2],
                    w[3],
                    out_shape[2],
                    out_shape[3],
                    stride[0],
                    stride[1],
                    pad[0],
                    pad[1],
                ];
                for (k, v) in vals.into_iter().enumerate() {
                    p[k] = u(v);
                }
                p[13] = act.code();
                let defs = if ins.len() > 2 {
                    "fn bias(o: u32) -> f32 { return ld2(o); }\n"
                } else {
                    "fn bias(o: u32) -> f32 { return 0.0; }\n"
                };
                let kinds: Vec<Bind> = ins.iter().map(|i| i.0).collect();
                let src = source(self.f16, false, &kinds, false, defs, false, CONV_TRANSPOSE);
                let groups = self.grid((n_out as u64).div_ceil(WG))?;
                self.push(src, ins, dst, groups, p);
            }
            Op::Softmax => {
                let d = *out_shape.last().expect("rank");
                let rows = n_out / d;
                p[0] = u(rows);
                p[1] = u(d);
                p[2] = 1f32.to_bits();
                p[4] = 1;
                self.push_rows(SOFTMAX, ins, dst, rows, p, false)?;
            }
            Op::Pool2d(pp) => {
                let s = shape(0);
                p[0] = u(n_out);
                p[1] = u(s[2]);
                p[2] = u(s[3]);
                p[3] = u(out_shape[2]);
                p[4] = u(out_shape[3]);
                p[5] = u(pp.kernel[0]);
                p[6] = u(pp.kernel[1]);
                p[7] = u(pp.stride[0]);
                p[8] = u(pp.stride[1]);
                p[9] = u(pp.pad[0]);
                p[10] = u(pp.pad[1]);
                p[11] = match pp.kind {
                    PoolKind::Max => 0,
                    PoolKind::Avg => 1,
                };
                self.push_1d(POOL, ins, dst, n_out, p)?;
            }
            Op::Reduce { op, axis } => {
                let s = shape(0);
                let len = s[*axis];
                let inner: usize = s[axis + 1..].iter().product();
                p[0] = u(n_out);
                p[1] = u(len);
                p[2] = u(inner);
                p[3] = match op {
                    ReduceOp::Sum => 0,
                    ReduceOp::Mean => 1,
                    ReduceOp::Max => 2,
                };
                // Long contiguous rows (a global pool over a large map): a workgroup per
                // output instead of one thread looping over the whole row.
                if inner == 1 && len >= ROW_REDUCE_MIN {
                    self.push_rows(REDUCE_ROWS, ins, dst, n_out, p, false)?;
                } else {
                    self.push_1d(REDUCE, ins, dst, n_out, p)?;
                }
            }
            Op::Resize { mode, .. } => {
                let s = shape(0);
                p[0] = u(n_out);
                p[1] = u(s[2]);
                p[2] = u(s[3]);
                p[3] = u(out_shape[2]);
                p[4] = u(out_shape[3]);
                p[5] = match mode {
                    ResizeMode::Nearest => 0,
                    ResizeMode::Bilinear => 1,
                    ResizeMode::BilinearAsymmetric | ResizeMode::BilinearAligned => 2,
                };
                let scale = if *mode == ResizeMode::BilinearAligned {
                    crate::cpu::aligned_scale
                } else {
                    resize_scale
                };
                p[6] = scale(s[2], out_shape[2]).to_bits();
                p[7] = scale(s[3], out_shape[3]).to_bits();
                self.push_1d(RESIZE, ins, dst, n_out, p)?;
            }
            Op::ReflectPad { pads } => {
                let s = shape(0);
                p[0] = u(n_out);
                p[1] = u(s[2]);
                p[2] = u(s[3]);
                p[3] = u(out_shape[2]);
                p[4] = u(out_shape[3]);
                p[5] = u(pads[0]);
                p[6] = u(pads[1]);
                self.push_1d(REFLECT_PAD, ins, dst, n_out, p)?;
            }
            Op::Dft(kind) => {
                let s = shape(0);
                let r = s.len();
                let (mode, n, q, sign, threads) = match *kind {
                    Dft::RealToComplex => (0, s[r - 1], s[r - 1] / 2 + 1, -1f32, n_out / 2),
                    Dft::Complex { inverse } => (
                        1,
                        s[r - 3],
                        s[r - 2],
                        if inverse { 1.0 } else { -1.0 },
                        n_out / 2,
                    ),
                    Dft::ComplexToReal { n } => (2, n, n / 2 + 1, 1.0, n_out),
                };
                p[0] = u(threads);
                p[1] = mode;
                p[2] = u(n);
                p[3] = u(q);
                p[4] = sign.to_bits();
                p[5] = (1.0 / (n as f32).sqrt()).to_bits();
                self.push_1d(DFT, ins, dst, threads, p)?;
            }
            Op::Gather => {
                let t = shape(0);
                p[0] = u(n_out);
                p[1] = u(t[0]);
                p[2] = u(t[1]);
                self.push_1d(GATHER, ins, dst, n_out, p)?;
            }
            Op::Lstm {
                reverse,
                summary,
                per,
            } => {
                let sp = shape(0);
                let ns = shape(1)[0];
                // The output (zeroed first: steps past a sequence's length stay zero), then
                // one dispatch per time step.
                p[0] = u(n_out);
                self.push_1d(ZERO, Vec::new(), dst, n_out, p)?;
                p[0] = u(sp[0]);
                p[1] = u(sp[1]);
                p[2] = u(ns);
                p[3] = u32::from(*reverse) | (u32::from(*summary) << 1);
                p[6] = u(n_out);
                let load = if ins.len() > 2 {
                    p[3] |= 4;
                    p[4] = u(*per);
                    "ld2(s / pu(4u))"
                } else {
                    "0.0"
                };
                let kinds: Vec<Bind> = ins.iter().map(|i| i.0).collect();
                // Sequences per workgroup: all of a small batch, at most LSTM_GEMM_SEQS;
                // states per workgroup to fill the threads. A few sequences (lines along
                // x): each sum split into slices, for more threads on a short step.
                let sq = sp[0].next_power_of_two().min(LSTM_GEMM_SEQS);
                let sl = if sp[0] >= LSTM_GEMM_SLICED_BELOW {
                    1
                } else {
                    LSTM_GEMM_SLICES
                };
                let un = (LSTM_GEMM_THREADS * sl.min(2) / (sq * sl)).clamp(1, LSTM_GEMM_UNITS);
                let body = LSTM_GEMM
                    .replace("LEN_LOAD", load)
                    .replace("WGS", &format!("{}u", un * sq * sl))
                    .replace("SLP", &format!("{}u", sl.saturating_sub(1).max(1)))
                    .replace("SL", &format!("{sl}u"))
                    .replace("SQ", &format!("{sq}u"))
                    .replace("UN", &format!("{un}u"))
                    .replace("KC", &format!("{LSTM_GEMM_K}u"));
                let src = source(self.f16, false, &kinds, false, "", false, &body);
                let groups = self.grid((sp[0].div_ceil(sq) * ns.div_ceil(un)) as u64)?;
                for k in 0..sp[1] {
                    p[5] = u(k);
                    self.push(src.clone(), ins.clone(), dst, groups, p);
                }
            }
            Op::CastF16 => {
                p[0] = u(n_out);
                let kinds = [ins[0].0];
                let src = source(self.f16, true, &kinds, false, "", false, CAST_F16);
                let groups = self.grid((n_out as u64).div_ceil(2).div_ceil(WG))?;
                self.push(src, ins, dst, groups, p);
            }
            Op::Preprocess(pp) => {
                let Def::Image(k) = g.info(node.inputs[0]).def else {
                    unreachable!("checked when built")
                };
                let groups = self.grid(((pp.width * pp.height) as u64).div_ceil(WG))?;
                let src = source(
                    false,
                    false,
                    &[Bind::Words],
                    false,
                    px_defs(false),
                    false,
                    PREPROCESS,
                );
                self.steps.push(StepPlan {
                    src,
                    ins: vec![Bind::Words],
                    res: vec![Res::Image],
                    dst,
                    groups,
                    params: p,
                    image: Some((k, *pp)),
                    label: self.label.clone(),
                    work: 0.0,
                });
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn copy(
        &mut self,
        src: (Bind, Res),
        dst: usize,
        dims: &[usize],
        src_strides: &[usize],
        src_base: usize,
        dst_strides: &[usize],
        dst_base: usize,
    ) -> Result<(), Error> {
        let n = numel(dims).unwrap_or(0);
        let mut p = [0u32; PARAM_WORDS];
        p[0] = u(n);
        p[1] = u(dims.len());
        p[2] = u(dst_base);
        p[3] = u(src_base);
        for d in 0..dims.len() {
            p[4 + d] = u(dims[d]);
            p[10 + d] = u(src_strides[d]);
            p[16 + d] = u(dst_strides[d]);
        }
        self.push_1d(COPY, vec![src], dst, n, p)
    }

    fn conv(
        &mut self,
        cp: &crate::graph::Conv2d,
        ins: Vec<(Bind, Res)>,
        dst: usize,
        (xs, ws, out_shape): (&[usize], &[usize], &[usize]),
        bias: bool,
        res: Option<usize>,
    ) -> Result<(), Error> {
        let (n, c, h, w) = (xs[0], xs[1], xs[2], xs[3]);
        let (o, cg, kh, kw) = (ws[0], ws[1], ws[2], ws[3]);
        let (oh, ow) = (out_shape[2], out_shape[3]);
        let g = cp.groups;
        let og = o / g;
        let pix = oh * ow;
        let pointwise =
            kh == 1 && kw == 1 && cp.stride == [1, 1] && cp.pad == [0; 4] && cp.dilation == [1, 1];
        if pointwise {
            // C[o, p] = W[o, c] · X[c, p] per (image, group).
            let mut ins = ins;
            ins.swap(0, 1);
            return self.push_strided_gemm(
                ins,
                dst,
                (og, pix, cg),
                (n, g),
                [0, 0, og * cg, cg, 1],
                [0, c * pix, cg * pix, pix, 1],
                [0, o * pix, og * pix, pix, 1],
                bias.then_some([og, 1, 0]),
                cp.act.code(),
                res,
            );
        }
        let mut p = [0u32; PARAM_WORDS];
        let vals = [
            og,
            pix,
            cg * kh * kw,
            g,
            c,
            h,
            w,
            kh,
            kw,
            ow,
            cp.stride[0],
            cp.stride[1],
            cp.pad[0],
            cp.pad[1],
            cp.dilation[0],
            cp.dilation[1],
            cg,
            oh,
            o,
        ];
        for (i, &v) in vals.iter().enumerate() {
            p[i] = u(v);
        }
        p[20] = cp.act.code();
        let total = n * o * pix;
        p[21] = u(total);
        let kinds: Vec<Bind> = ins.iter().map(|i| i.0).collect();
        if direct(xs, ws, g) {
            let defs = if bias {
                "fn bias_at(o: u32) -> f32 { return ld2(o); }\n"
            } else {
                "fn bias_at(o: u32) -> f32 { return 0.0; }\n"
            };
            let src = source(self.f16, false, &kinds, false, defs, false, CONV_DIRECT);
            let groups = self.grid((total as u64).div_ceil(WG))?;
            self.push(src, ins, dst, groups, p);
            return Ok(());
        }
        let a_vec = (cg * kh * kw) % 4 == 0;
        let mut kinds = kinds;
        let mut ins = ins;
        if a_vec {
            ins[1].0 = ins[1].0.vec4();
            kinds[1] = ins[1].0;
        }
        let mut defs = gemm_order(true, true);
        defs.push_str(&conv_defs(bias, res, a_vec));
        let src = if a_vec {
            wide_source(self.f16, &kinds, &defs, true, false)
        } else {
            source(self.f16, false, &kinds, false, &defs, false, GEMM)
        };
        let groups = self.gemm_grid(og, pix, n * g)?;
        self.push_gemm(src, ins, dst, groups, p, og * pix * cg * kh * kw * n * g);
        Ok(())
    }
}

/// Whether a convolution runs on the direct kernel (one thread per output) rather than the
/// GEMM: depthwise ones, and those with a few output channels per group, which would fill
/// a few rows of the GEMM's 64-row tiles (Big-LaMa's last 7×7 convolution, 64 → 3 channels
/// at 448²: 239 ms on the GEMM).
fn direct(xs: &[usize], ws: &[usize], groups: usize) -> bool {
    let depthwise = groups == xs[1] && ws[1] == 1 && ws[0] == groups;
    depthwise || ws[0] / groups.max(1) <= DIRECT_MAX_OUT
}

/// A transpose that swaps two runs of axes after leading ones it keeps: `[B, R, C]` to
/// `[B, C, R]` with the runs' sizes multiplied out (B, R, C), or None.
fn batched_swap(shape: &[usize], perm: &[usize]) -> Option<(usize, usize, usize)> {
    let keep = perm
        .iter()
        .enumerate()
        .take_while(|&(i, &p)| i == p)
        .count();
    let rest = &perm[keep..];
    let m = *rest.first()?;
    // rest = [m, m + 1, …, r − 1, keep, keep + 1, …, m − 1].
    let r = perm.len();
    let ok = m > keep && rest.iter().copied().eq((m..r).chain(keep..m));
    ok.then(|| {
        let prod = |a: usize, b: usize| shape[a..b].iter().product::<usize>();
        (prod(0, keep), prod(keep, m), prod(m, r))
    })
}

/// Values a transpose moves at least to go through tiles rather than the strided copy.
const TRANSPOSE_2D_MIN: usize = 65_536;

/// Multiply-adds of one GEMM dispatch at most (about 20 ms on a Radeon 860M) before it is
/// split along its rows of tiles.
const GEMM_SPLIT_MACS: usize = 4_000_000_000;

/// Output channels per group at most for the direct convolution kernel.
const DIRECT_MAX_OUT: usize = 4;

/// Residual adds that can run in the epilogue of the GEMM producing one operand: per node,
/// the add's output and other operand, and which nodes (the adds) are then skipped. The
/// product's output must be f32, read only by the add, not a graph output, and of the add's
/// shape; the other operand must be f32 of that shape too and exist before the product.
fn residual_fusion(g: &Graph) -> (Vec<Option<(Value, Value)>>, Vec<bool>) {
    let n = g.nodes.len();
    let mut fused = vec![None; n];
    let mut skip = vec![false; n];
    let mut readers = vec![0usize; g.values.len()];
    for node in &g.nodes {
        for v in &node.inputs {
            readers[v.index()] += 1;
        }
    }
    let gemm = |j: usize| match &g.nodes[j].op {
        Op::MatMul(_) => true,
        Op::Conv2d(cp) => !direct(
            g.shape(g.nodes[j].inputs[0]),
            g.shape(g.nodes[j].inputs[1]),
            cp.groups,
        ),
        _ => false,
    };
    for (i, node) in g.nodes.iter().enumerate() {
        if node.op != Op::Binary(crate::graph::BinaryOp::Add) || node.inputs.len() != 2 {
            continue;
        }
        let out = node.output;
        let shape = g.shape(out);
        if g.dtype(out) != DType::F32 {
            continue;
        }
        for (x, y) in [
            (node.inputs[0], node.inputs[1]),
            (node.inputs[1], node.inputs[0]),
        ] {
            let Def::Node(j) = g.info(x).def else {
                continue;
            };
            let before = match g.info(y).def {
                Def::Node(k) => k < j,
                Def::Image(_) => false,
                Def::Input(_) | Def::Const(_) => true,
            };
            if x != y
                && gemm(j)
                && fused[j].is_none()
                && readers[x.index()] == 1
                && !g.outputs.contains(&x)
                && g.dtype(x) == DType::F32
                && g.dtype(y) == DType::F32
                && g.shape(x) == shape
                && g.shape(y) == shape
                && before
            {
                fused[j] = Some((out, y));
                skip[i] = true;
                break;
            }
        }
    }
    (fused, skip)
}

/// A short name of what a node computes: the op and its output shape.
fn op_label(op: &Op, shape: &[usize]) -> String {
    let name = match op {
        Op::Conv2d(c) if c.groups > 1 => "conv2d grouped",
        Op::Conv2d(_) => "conv2d",
        Op::MatMul(_) => "matmul",
        Op::Attention { .. } => "attention",
        Op::LayerNorm { .. } => "layer norm",
        Op::ChannelNorm { .. } => "channel norm",
        Op::GroupNorm { .. } => "group norm",
        Op::Gate => "gate",
        Op::DepthToSpace { .. } => "depth to space",
        Op::ConvTranspose2d { .. } => "conv transpose",
        Op::L2Norm { .. } => "l2 norm",
        Op::Softmax => "softmax",
        Op::Binary(_) => "binary",
        Op::Unary { .. } => "unary",
        Op::Transpose { .. } => "transpose",
        Op::Slice { .. } => "slice",
        Op::Concat { .. } => "concat",
        Op::Pool2d(_) => "pool",
        Op::Reduce { .. } => "reduce",
        Op::Resize { .. } => "resize",
        Op::Gather => "gather",
        Op::CastF16 => "cast f16",
        Op::Lstm { .. } => "lstm",
        Op::ReflectPad { .. } => "reflect pad",
        Op::Dft(_) => "dft",
        Op::Preprocess(_) => "preprocess",
        Op::Reshape => "reshape",
    };
    format!("{name} {shape:?}")
}

/// A step ready to record.
struct Step {
    label: String,
    pipeline: Pipeline,
    bind: Option<wgpu::BindGroup>,
    groups: [u32; 3],
    /// For image steps: the graph input, its parameters and the buffer it writes.
    image: Option<(usize, Preprocess, usize)>,
    /// Estimated work units, for a time estimate before the label's time is learnt.
    work: f64,
}

/// Timestamps a recording writes.
#[derive(Clone, Copy)]
enum Timing<'a> {
    None,
    /// Around the one pass, at queries 2k and 2k + 1.
    Pass(&'a wgpu::QuerySet, u32),
    /// Around every dispatch (each in its own pass), at 2i and 2i + 1 for dispatch i.
    PerDispatch(&'a wgpu::QuerySet),
}

/// How a run went on the GPU ([`GpuGraph::last_run`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunStats {
    /// Queue submissions the run went in.
    pub submissions: usize,
    /// The longest submission: milliseconds from when it could start (the previous one
    /// finished, or the run was submitted) to when it finished.
    pub longest_ms: f64,
    /// From the first submission to the last one finished.
    pub total_ms: f64,
    /// The longest submission as estimated beforehand.
    pub estimated_longest_ms: f64,
    /// The longest submission's GPU time from timestamps, when the device has them.
    pub gpu_longest_ms: Option<f64>,
    /// The submissions' GPU time added up (what one submission would have held), when the
    /// device has timestamps.
    pub gpu_total_ms: Option<f64>,
}

/// Runs on a context ([`Context::take_stats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunTotals {
    pub runs: usize,
    pub submissions: usize,
    /// The longest submission (GPU time when timed, else as [`RunStats::longest_ms`]).
    pub longest_ms: f64,
    /// The longest run's GPU time when timed, else its time from first submission to last
    /// finished.
    pub longest_run_ms: f64,
}

/// How a graph input is fed.
enum Input {
    Tensor {
        slot: usize,
        shape: Vec<usize>,
        dtype: DType,
    },
    Image,
}

/// A graph compiled for one device. Build once, [`run`](GpuGraph::run) many times.
pub struct GpuGraph {
    ctx: Context,
    buffers: Vec<wgpu::Buffer>,
    uniform: wgpu::Buffer,
    steps: Vec<Step>,
    inputs: Vec<Input>,
    /// Per graph input: the RGBA8 upload buffer and its size.
    images: Vec<Option<(wgpu::Buffer, u64)>>,
    outputs: Vec<(usize, Vec<usize>, DType, u64)>,
    readback: wgpu::Buffer,
    readback_bytes: u64,
    bytes: u64,
    /// Estimated GPU milliseconds a submission may hold (else the context's).
    submit_ms: Option<f64>,
    last: RunStats,
}

/// Runs `f` inside validation and out-of-memory error scopes.
fn scoped<T>(device: &wgpu::Device, f: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    let oom = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let val = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let r = f();
    let e1 = pollster::block_on(val.pop());
    let e2 = pollster::block_on(oom.pop());
    match (e1, e2) {
        (Some(e), _) | (None, Some(e)) => Err(Error::Gpu(e.to_string())),
        _ => r,
    }
}

impl GpuGraph {
    pub fn new(ctx: &Context, graph: &Graph) -> Result<GpuGraph, Error> {
        ctx.guard(|| scoped(&ctx.inner.device, || GpuGraph::build(ctx, graph)))
    }

    fn build(ctx: &Context, graph: &Graph) -> Result<GpuGraph, Error> {
        let device = &ctx.inner.device;
        let limits = &ctx.inner.limits;
        let (fused, skip) = residual_fusion(graph);
        let mut pl = Planner {
            g: graph,
            max_groups: u64::from(limits.max_compute_workgroups_per_dimension),
            max_binding: limits
                .max_storage_buffer_binding_size
                .min(limits.max_buffer_size),
            slots: Vec::new(),
            free: Vec::new(),
            vslot: vec![None; graph.values.len()],
            steps: Vec::new(),
            label: String::new(),
            f16: ctx.inner.features.f16,
            subgroups: ctx.inner.features.subgroups,
            rows_wg: ctx.inner.rows_wg,
            sg_min: ctx.inner.sg_min,
            fused,
            skip,
        };
        // Constants and inputs: their own buffers.
        let mut consts = Vec::new();
        let mut inputs = Vec::new();
        for (i, info) in graph.values.iter().enumerate() {
            match &info.def {
                Def::Const(t) => {
                    let s = pl.dedicated(value_bytes(&info.shape, info.dtype))?;
                    pl.vslot[i] = Some(s);
                    consts.push((s, t));
                }
                Def::Input(_) => {
                    let s = pl.dedicated(value_bytes(&info.shape, info.dtype))?;
                    pl.vslot[i] = Some(s);
                }
                Def::Image(_) | Def::Node(_) => {}
            }
        }
        for &v in &graph.inputs {
            let info = graph.info(v);
            inputs.push(match info.def {
                Def::Image(_) => Input::Image,
                _ => Input::Tensor {
                    slot: pl.vslot[v.index()].expect("input"),
                    shape: info.shape.clone(),
                    dtype: info.dtype,
                },
            });
        }
        // Liveness by buffer owner (a reshape's owner is its input's).
        let n = graph.values.len();
        let mut owner: Vec<usize> = (0..n).collect();
        for node in &graph.nodes {
            if node.op == Op::Reshape {
                owner[node.output.index()] = owner[node.inputs[0].index()];
            }
        }
        let mut last = vec![None; n];
        for (i, node) in graph.nodes.iter().enumerate() {
            for v in &node.inputs {
                last[owner[v.index()]] = Some(i);
            }
        }
        let mut keep = vec![false; n];
        for v in &graph.outputs {
            keep[owner[v.index()]] = true;
        }
        for (i, node) in graph.nodes.iter().enumerate() {
            let first = pl.steps.len();
            pl.node(i)?;
            // The node's work shared among its steps; each step's label gets the first of
            // the node's steps running the same kernel (an LSTM's time steps share one): the
            // key its time is learnt by.
            let made = pl.steps.len() - first;
            let work = split::node_work(graph, node) / made.max(1) as f64;
            for k in first..pl.steps.len() {
                let same = (first..k)
                    .find(|&j| pl.steps[j].src == pl.steps[k].src)
                    .unwrap_or(k);
                pl.steps[k].work = work;
                let _ = write!(pl.steps[k].label, " #{}", same - first);
            }
            let mut dead: Vec<usize> = node
                .inputs
                .iter()
                .map(|v| owner[v.index()])
                .chain(std::iter::once(owner[node.output.index()]))
                .filter(|&o| {
                    !keep[o]
                        && matches!(graph.values[o].def, Def::Node(_))
                        && last[o].is_none_or(|l| l == i)
                })
                .collect();
            dead.sort_unstable();
            dead.dedup();
            // Inputs read for the last time, and an output nobody reads.
            for o in dead {
                if let Some(s) = pl.vslot[o] {
                    pl.free.push(s);
                }
            }
        }
        // Buffers.
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let buffers: Vec<wgpu::Buffer> = pl
            .slots
            .iter()
            .map(|&size| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("infer value"),
                    // Whole vec4s, for kernels reading four values at a time.
                    size: size.next_multiple_of(16),
                    usage,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let bytes = pl.slots.iter().sum();
        for (s, t) in consts {
            ctx.inner.queue.write_buffer(&buffers[s], 0, &t.to_bytes());
        }
        // Parameters, pipelines, bind groups.
        let mut words: Vec<u32> = Vec::with_capacity(pl.steps.len().max(1) * PARAM_WORDS);
        for st in &pl.steps {
            words.extend_from_slice(&st.params);
        }
        if words.is_empty() {
            words.resize(PARAM_WORDS, 0);
        }
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("infer params"),
            size: (words.len() * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.inner
            .queue
            .write_buffer(&uniform, 0, bytemuck::cast_slice(&words));
        let mut steps = Vec::with_capacity(pl.steps.len());
        for (si, st) in pl.steps.iter().enumerate() {
            let pipeline = ctx.pipeline(&st.src, &st.ins)?;
            let bind = if st.image.is_some() {
                None
            } else {
                let res: Vec<&wgpu::Buffer> = st
                    .res
                    .iter()
                    .map(|r| match r {
                        Res::Slot(s) => &buffers[*s],
                        Res::Image => unreachable!("image steps bind per run"),
                    })
                    .collect();
                Some(bind_group(
                    device,
                    &pipeline.layout,
                    &uniform,
                    si,
                    &buffers[st.dst],
                    &res,
                    None,
                ))
            };
            steps.push(Step {
                label: st.label.clone(),
                work: st.work,
                pipeline,
                bind,
                groups: st.groups,
                image: st.image.map(|(k, p)| (k, p, st.dst)),
            });
        }
        // Outputs and their readback.
        let mut outputs = Vec::new();
        let mut off = 0u64;
        for &v in &graph.outputs {
            let info = graph.info(v);
            let b = value_bytes(&info.shape, info.dtype);
            let slot = pl.vslot[v.index()].expect("output planned");
            outputs.push((slot, info.shape.clone(), info.dtype, b));
            off += b;
        }
        let readback_bytes = off.max(4);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("infer readback"),
            size: readback_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(GpuGraph {
            ctx: ctx.clone(),
            buffers,
            uniform,
            steps,
            images: (0..inputs.len()).map(|_| None).collect(),
            inputs,
            outputs,
            readback,
            readback_bytes,
            bytes,
            submit_ms: None,
            last: RunStats::default(),
        })
    }

    /// Sets the estimated GPU milliseconds one queue submission of this graph may hold
    /// (else the context's, [`Context::set_submission_ms`]); 0 submits every dispatch on its
    /// own, infinity the whole run at once.
    pub fn set_submission_ms(&mut self, ms: f64) {
        self.submit_ms = Some(ms.max(0.0));
    }

    fn limit(&self) -> f64 {
        self.submit_ms.unwrap_or_else(|| self.ctx.submission_ms())
    }

    /// How the last [`run`](GpuGraph::run) went.
    pub fn last_run(&self) -> RunStats {
        self.last
    }

    /// Each dispatch's estimated GPU milliseconds, in order.
    fn estimates(&self) -> Vec<f64> {
        let speeds = self.ctx.speeds();
        self.steps
            .iter()
            .map(|st| speeds.estimate(&st.label, st.work).0)
            .collect()
    }

    /// Bytes of device memory the graph's values use (weights included).
    pub fn memory_bytes(&self) -> u64 {
        self.bytes
    }

    /// Number of compute dispatches per run.
    pub fn dispatches(&self) -> usize {
        self.steps.len()
    }

    /// Runs the graph: uploads the feeds, the dispatches in submissions of at most the
    /// submission limit's estimated GPU time ([`set_submission_ms`]), reads the outputs back.
    ///
    /// [`set_submission_ms`]: GpuGraph::set_submission_ms
    pub fn run(&mut self, feeds: &[Feed]) -> Result<Vec<Tensor>, Error> {
        if feeds.len() != self.inputs.len() {
            return Err(Error::Feed(format!(
                "{} inputs fed, the graph has {}",
                feeds.len(),
                self.inputs.len()
            )));
        }
        let ctx = self.ctx.clone();
        ctx.guard(|| scoped(&ctx.inner.device, || self.run_inner(feeds)))
    }

    fn run_inner(&mut self, feeds: &[Feed]) -> Result<Vec<Tensor>, Error> {
        self.prepare(feeds)?;
        let ctx = self.ctx.clone();
        let (device, queue) = (&ctx.inner.device, &ctx.inner.queue);
        let est = self.estimates();
        let ranges = split::submissions(&est, self.limit());
        let last = ranges.len() - 1;
        // Each submission's GPU time, when the device has timestamps.
        let queries = (2 * ranges.len()) as u32;
        let stamps = (ctx.inner.features.timestamps && queries <= wgpu::QUERY_SET_MAX_QUERIES)
            .then(|| timestamp_buffers(device, queries));
        let mut bufs = Vec::with_capacity(ranges.len());
        for (k, range) in ranges.iter().enumerate() {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("infer run"),
            });
            let timing = match &stamps {
                Some((qs, ..)) => Timing::Pass(qs, k as u32),
                None => Timing::None,
            };
            self.record(&mut enc, range.clone(), timing)?;
            if k == last {
                let mut off = 0;
                for (slot, _, _, b) in &self.outputs {
                    enc.copy_buffer_to_buffer(&self.buffers[*slot], 0, &self.readback, off, *b);
                    off += b;
                }
                if let Some((qs, resolve, read)) = &stamps {
                    enc.resolve_query_set(qs, 0..queries, resolve, 0);
                    enc.copy_buffer_to_buffer(resolve, 0, read, 0, u64::from(queries) * 8);
                }
            }
            bufs.push(enc.finish());
        }
        // Up together (the GPU runs them back to back), then each one's finishing time.
        let t0 = std::time::Instant::now();
        let ids: Vec<wgpu::SubmissionIndex> = bufs.into_iter().map(|b| queue.submit([b])).collect();
        let mut took = Vec::with_capacity(ids.len());
        let mut prev = t0;
        for id in ids {
            ctx.poll(wgpu::PollType::Wait {
                submission_index: Some(id),
                timeout: None,
            })?;
            let now = std::time::Instant::now();
            took.push(now.duration_since(prev).as_secs_f64() * 1e3);
            prev = now;
        }
        let gpu_ms: Option<Vec<f64>> = match &stamps {
            Some((_, _, read)) => {
                let bytes = u64::from(queries) * 8;
                let ticks: Vec<u64> = {
                    let view = map_read(&ctx, read, bytes)?;
                    bytemuck::pod_collect_to_vec(&view[..])
                };
                read.unmap();
                let period = f64::from(queue.get_timestamp_period()) * 1e-6;
                Some(
                    ticks
                        .chunks_exact(2)
                        .map(|t| t[1].saturating_sub(t[0]) as f64 * period)
                        .collect(),
                )
            }
            None => None,
        };
        {
            let mut speeds = ctx.speeds();
            let times = gpu_ms.as_ref().unwrap_or(&took);
            for (range, &ms) in ranges.iter().zip(times) {
                speeds.learn(
                    self.steps[range.clone()]
                        .iter()
                        .map(|st| (st.label.as_str(), st.work)),
                    ms,
                );
            }
        }
        self.last = RunStats {
            submissions: ranges.len(),
            longest_ms: took.iter().copied().fold(0.0, f64::max),
            total_ms: prev.duration_since(t0).as_secs_f64() * 1e3,
            estimated_longest_ms: ranges
                .iter()
                .map(|r| est[r.clone()].iter().sum::<f64>())
                .fold(0.0, f64::max),
            gpu_longest_ms: gpu_ms
                .as_ref()
                .map(|v| v.iter().copied().fold(0.0, f64::max)),
            gpu_total_ms: gpu_ms.map(|v| v.into_iter().sum()),
        };
        {
            let mut s = ctx.inner.stats.lock().unwrap_or_else(|e| e.into_inner());
            let l = &self.last;
            s.runs += 1;
            s.submissions += l.submissions;
            s.longest_ms = s.longest_ms.max(l.gpu_longest_ms.unwrap_or(l.longest_ms));
            s.longest_run_ms = s.longest_run_ms.max(l.gpu_total_ms.unwrap_or(l.total_ms));
        }
        let view = map_read(&ctx, &self.readback, self.readback_bytes)?;
        let mut out = Vec::with_capacity(self.outputs.len());
        let mut off = 0usize;
        for (_, shape, dtype, b) in &self.outputs {
            let n = numel(shape).unwrap_or(0);
            let bytes = &view[off..off + *b as usize];
            out.push(match dtype {
                DType::F32 => Tensor::f32(shape, bytemuck::pod_collect_to_vec(&bytes[..n * 4])),
                DType::F16 => {
                    Tensor::f16_bits(shape, bytemuck::pod_collect_to_vec(&bytes[..n * 2]))
                }
            });
            off += *b as usize;
        }
        drop(view);
        self.readback.unmap();
        Ok(out)
    }

    /// Runs the graph once with every dispatch in its own compute pass, timed with GPU
    /// timestamps: (step label, milliseconds) per dispatch, in order. Needs
    /// [`Features::timestamps`]. The outputs are computed but not read back.
    pub fn profile(&mut self, feeds: &[Feed]) -> Result<Vec<(String, f64)>, Error> {
        if !self.ctx.inner.features.timestamps {
            return gpu_err("the device has no timestamp queries");
        }
        if feeds.len() != self.inputs.len() {
            return Err(Error::Feed("wrong number of inputs".into()));
        }
        let ctx = self.ctx.clone();
        let (device, queue) = (&ctx.inner.device, &ctx.inner.queue);
        ctx.guard(|| {
            scoped(device, || {
                self.prepare(feeds)?;
                let n = self.steps.len();
                let queries = u32::try_from(2 * n.max(1))
                    .ok()
                    .filter(|&q| q <= wgpu::QUERY_SET_MAX_QUERIES)
                    .ok_or(Error::Gpu("too many dispatches to profile".into()))?;
                let qs = device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("infer profile"),
                    ty: wgpu::QueryType::Timestamp,
                    count: queries,
                });
                let bytes = u64::from(queries) * 8;
                let resolve = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("infer profile"),
                    size: bytes,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let read = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("infer profile read"),
                    size: bytes,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let ranges = split::submissions(&self.estimates(), self.limit());
                let last = ranges.len() - 1;
                for (k, range) in ranges.into_iter().enumerate() {
                    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("infer profile"),
                    });
                    self.record(&mut enc, range, Timing::PerDispatch(&qs))?;
                    if k == last {
                        enc.resolve_query_set(&qs, 0..queries, &resolve, 0);
                        enc.copy_buffer_to_buffer(&resolve, 0, &read, 0, bytes);
                    }
                    queue.submit([enc.finish()]);
                }
                let view = map_read(&ctx, &read, bytes)?;
                let ticks: Vec<u64> = bytemuck::pod_collect_to_vec(&view[..]);
                drop(view);
                read.unmap();
                let period = f64::from(queue.get_timestamp_period()) * 1e-6;
                let times: Vec<(String, f64)> = self
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(i, st)| {
                        let d = ticks[2 * i + 1].saturating_sub(ticks[2 * i]);
                        (st.label.clone(), d as f64 * period)
                    })
                    .collect();
                ctx.speeds().measured(&times);
                Ok(times)
            })
        })
    }

    /// Records the dispatches in `range`: one compute pass, or one pass per dispatch with
    /// timestamps written around each when profiling.
    fn record(
        &self,
        enc: &mut wgpu::CommandEncoder,
        range: std::ops::Range<usize>,
        timing: Timing,
    ) -> Result<(), Error> {
        if self.steps[range.clone()].iter().any(|st| st.bind.is_none()) {
            return Err(Error::Gpu("a dispatch has no bindings".into()));
        }
        let dispatch = |cp: &mut wgpu::ComputePass, st: &Step| {
            cp.set_pipeline(&st.pipeline.pipeline);
            cp.set_bind_group(0, st.bind.as_ref(), &[]);
            cp.dispatch_workgroups(st.groups[0], st.groups[1], st.groups[2]);
        };
        match timing {
            Timing::None | Timing::Pass(..) => {
                let writes = match timing {
                    Timing::Pass(qs, k) => Some(wgpu::ComputePassTimestampWrites {
                        query_set: qs,
                        beginning_of_pass_write_index: Some(2 * k),
                        end_of_pass_write_index: Some(2 * k + 1),
                    }),
                    _ => None,
                };
                let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("infer run"),
                    timestamp_writes: writes,
                });
                for st in &self.steps[range] {
                    dispatch(&mut cp, st);
                }
            }
            Timing::PerDispatch(qs) => {
                for (i, st) in self
                    .steps
                    .iter()
                    .enumerate()
                    .take(range.end)
                    .skip(range.start)
                {
                    let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("infer profile"),
                        timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                            query_set: qs,
                            beginning_of_pass_write_index: Some(2 * i as u32),
                            end_of_pass_write_index: Some(2 * i as u32 + 1),
                        }),
                    });
                    dispatch(&mut cp, st);
                }
            }
        }
        Ok(())
    }

    /// Uploads the feeds and sets up the image steps for them.
    fn prepare(&mut self, feeds: &[Feed]) -> Result<(), Error> {
        let ctx = self.ctx.clone();
        let (device, queue) = (&ctx.inner.device, &ctx.inner.queue);
        // Image sources for this run: (buffer or texture view, width, height).
        let mut images: Vec<Option<(ImageSrc, u32, u32)>> =
            (0..feeds.len()).map(|_| None).collect();
        for (k, (feed, input)) in feeds.iter().zip(&self.inputs).enumerate() {
            match (input, feed) {
                (Input::Tensor { slot, shape, dtype }, Feed::Tensor(t))
                    if &t.shape == shape && t.dtype() == *dtype =>
                {
                    match &t.data {
                        Data::F32(v) if !v.is_empty() => {
                            queue.write_buffer(&self.buffers[*slot], 0, bytemuck::cast_slice(v));
                        }
                        _ => queue.write_buffer(&self.buffers[*slot], 0, &t.to_bytes()),
                    }
                }
                (Input::Tensor { shape, dtype, .. }, _) => {
                    return Err(Error::Feed(format!(
                        "input {k} wants a {dtype:?} tensor {shape:?}"
                    )));
                }
                (
                    Input::Image,
                    Feed::Rgba {
                        width,
                        height,
                        data,
                    },
                ) => {
                    let len = u64::from(*width) * u64::from(*height) * 4;
                    if *width == 0 || *height == 0 || data.len() as u64 != len {
                        return Err(Error::Feed(format!("image {k}: bad size")));
                    }
                    if len > ctx.inner.limits.max_storage_buffer_binding_size {
                        return Err(Error::Feed(format!("image {k} is too large")));
                    }
                    let fits = self.images[k].as_ref().is_some_and(|(_, cap)| *cap >= len);
                    if !fits {
                        let b = device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("infer image"),
                            size: len,
                            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        self.images[k] = Some((b, len));
                    }
                    let (b, _) = self.images[k].as_ref().expect("image buffer");
                    queue.write_buffer(b, 0, data);
                    images[k] = Some((ImageSrc::Buffer(b.clone()), *width, *height));
                }
                (Input::Image, Feed::Texture(t)) => {
                    if t.format() != wgpu::TextureFormat::Rgba8Unorm
                        || !t.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING)
                        || t.dimension() != wgpu::TextureDimension::D2
                    {
                        return Err(Error::Feed(format!(
                            "image {k}: the texture must be a sampled 2-D Rgba8Unorm"
                        )));
                    }
                    let view = t.create_view(&wgpu::TextureViewDescriptor::default());
                    images[k] = Some((ImageSrc::Texture(view), t.width(), t.height()));
                }
                (Input::Image, Feed::Tensor(_)) => {
                    return Err(Error::Feed(format!("input {k} wants an image")));
                }
            }
        }
        // Image steps: per-run parameters, kernel variant and bind group.
        for si in 0..self.steps.len() {
            let Some((k, pp, dst)) = self.steps[si].image else {
                continue;
            };
            let (src, w, h) = images[k].as_ref().expect("image fed");
            let [x0, y0, sx, sy, fx, fy] = preprocess_params(&pp, *w as usize, *h as usize);
            let mut p = [0u32; PARAM_WORDS];
            p[0] = u(pp.width);
            p[1] = u(pp.height);
            p[2] = *w;
            p[3] = *h;
            for (i, f) in [x0, y0, sx, sy, fx, fy].iter().enumerate() {
                p[4 + i] = f.to_bits();
            }
            for c in 0..3 {
                p[10 + c] = pp.mean[c].to_bits();
                p[13 + c] = pp.std[c].to_bits();
            }
            queue.write_buffer(
                &self.uniform,
                si as u64 * PARAM_BYTES,
                bytemuck::cast_slice(&p),
            );
            let texture = matches!(src, ImageSrc::Texture(_));
            let kind = if texture { Bind::Texture } else { Bind::Words };
            let code = source(
                false,
                false,
                &[kind],
                false,
                px_defs(texture),
                false,
                PREPROCESS,
            );
            let pipeline = ctx.pipeline(&code, &[kind])?;
            let bind = match src {
                ImageSrc::Buffer(b) => bind_group(
                    device,
                    &pipeline.layout,
                    &self.uniform,
                    si,
                    &self.buffers[dst],
                    &[b],
                    None,
                ),
                ImageSrc::Texture(v) => bind_group(
                    device,
                    &pipeline.layout,
                    &self.uniform,
                    si,
                    &self.buffers[dst],
                    &[],
                    Some(v),
                ),
            };
            self.steps[si].pipeline = pipeline;
            self.steps[si].bind = Some(bind);
        }
        Ok(())
    }
}

/// `device.poll(ty)`, catching wgpu's panic on a lost device: [`Error::Lost`] then
/// ([`Context::poll`] for a context).
pub fn poll_device(device: &wgpu::Device, ty: wgpu::PollType) -> Result<(), Error> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| device.poll(ty))) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(Error::Gpu(e.to_string())),
        Err(_) => Err(Error::Lost),
    }
}

/// `wgpu::util::DeviceExt::create_buffer_init` that survives a lost device (wgpu's expects
/// the mapping a lost device refuses, a panic): the buffer is then left unfilled.
pub fn buffer_init(
    device: &wgpu::Device,
    desc: &wgpu::util::BufferInitDescriptor<'_>,
) -> wgpu::Buffer {
    let len = desc.contents.len() as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: desc.label,
        size: len.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
        usage: desc.usage,
        mapped_at_creation: len > 0,
    });
    if len > 0 {
        if let Ok(mut range) = buffer.get_mapped_range_mut(..) {
            range.slice(..len as usize).copy_from_slice(desc.contents);
        }
        buffer.unmap();
    }
    buffer
}

/// A timestamp query set of `queries`, its resolve buffer and a buffer to read it from.
fn timestamp_buffers(
    device: &wgpu::Device,
    queries: u32,
) -> (wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer) {
    let bytes = u64::from(queries) * 8;
    let qs = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("infer timing"),
        ty: wgpu::QueryType::Timestamp,
        count: queries,
    });
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("infer timing"),
        size: bytes,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("infer timing read"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    (qs, resolve, read)
}

/// Maps `bytes` of `buffer` for reading and waits for it.
fn map_read(ctx: &Context, buffer: &wgpu::Buffer, bytes: u64) -> Result<wgpu::BufferView, Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..bytes)
        .map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r.is_ok());
        });
    ctx.poll(wgpu::PollType::wait_indefinitely())?;
    // Another thread polling the same device may have collected the callback and not run it
    // yet when this poll returns: wait for it rather than read the flag.
    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(true) => {}
        _ => return Err(Error::Gpu("reading the outputs back failed".into())),
    }
    buffer
        .slice(..bytes)
        .get_mapped_range()
        .map_err(|e| Error::Gpu(format!("{e:?}")))
}

enum ImageSrc {
    Buffer(wgpu::Buffer),
    Texture(wgpu::TextureView),
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    step: usize,
    dst: &wgpu::Buffer,
    ins: &[&wgpu::Buffer],
    texture: Option<&wgpu::TextureView>,
) -> wgpu::BindGroup {
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: uniform,
                offset: step as u64 * PARAM_BYTES,
                size: std::num::NonZeroU64::new(PARAM_BYTES),
            }),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: dst.as_entire_binding(),
        },
    ];
    for (k, b) in ins.iter().enumerate() {
        entries.push(wgpu::BindGroupEntry {
            binding: k as u32 + 2,
            resource: b.as_entire_binding(),
        });
    }
    if let Some(v) = texture {
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::TextureView(v),
        });
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("infer step"),
        layout,
        entries: &entries,
    })
}
