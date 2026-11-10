//! Per-machine choice between the CPU and GPU (or other) ways of doing the same work: the
//! `PathSelector` of DESIGN.md §12.3.1.
//!
//! Every dual path registers as a [`Task`] whose [`Engine`]s are fixed: index 0 is always the
//! pure-Rust CPU reference, which is always available and always trusted. Each machine picks
//! between them from its own timings; fixed numbers ([`prior`]) are only the starting guess.
//!
//! * **Probe.** The owners of the other paths say what exists ([`PathSelector::set_trust`]),
//!   with the device class ([`GpuClass`]) and CPU threads for the priors. The probe and
//!   everything learned are cached per GPU, driver, CPU and app version ([`fingerprint`]).
//! * **Calibrate.** The owners time each path on built-in inputs while the machine is idle and
//!   record the timings like any other run; [`PathSelector::calibrated`] says whether that
//!   happened for this fingerprint.
//! * **Learn.** [`PathSelector::start`] returns a [`Run`] that times one real run end to end
//!   and records it: wall time per megapixel above a fixed setup cost (a machine faster than
//!   the prior overall is faster at both, see [`PathSelector::record`]), and CPU time over
//!   all the threads that worked for it (exact when it ran alone, shared by weight when
//!   tracked work overlapped; see `paths/cpu.rs`). Cells are per path, variant (format
//!   detail, e.g. compression ratio), size bucket and load (CPU idle or busy).
//! * **Decide** ([`PathSelector::choose`]) per [`Request`]: interactive requests take the
//!   lowest predicted time; background ones the lowest CPU time that still keeps up; on
//!   battery or under thermal pressure, the lowest energy. Every sixteenth background request
//!   per task explores a path not currently chosen; interactive requests never do.
//! * **Safety rails.** A non-reference path is used only once [`Trust::Trusted`] (its owner
//!   checked it against the CPU reference); until then a request that can check
//!   ([`Request::can_verify`]) may get it with [`Choice::verify`] set. An error blocklists the
//!   path ([`Run::fail`], [`PathSelector::block`]) for this fingerprint.
//! * **Overridable.** [`Mode`] (Automatic, Prefer GPU, CPU only) and a per-task [`Override`].
//!
//! [`PathSelector::choose`] is O(1) and allocation-free: a fixed table of atomics, at most
//! [`MAX_PATHS`] predictions, each a bounded lookup. Timing tables ([`PathSelector::to_table`],
//! [`PathSelector::load_table`]) are the cache file's format and also what tests replay.

use std::cell::Cell;
use std::fmt::Write as _;
use std::sync::atomic::{
    AtomicBool, AtomicU8, AtomicU16, AtomicU32, AtomicU64, AtomicUsize, Ordering,
};
use std::time::Instant;

use parking_lot::Mutex;

// CPU time of a run over all its threads (cpu.rs).
mod cpu;
// Read-only snapshot for Settings → Performance, retry and "Measure again" (snapshot.rs).
mod snapshot;
pub use snapshot::*;

/// At most this many ways of doing one task.
pub const MAX_PATHS: usize = 3;
/// Variants per task (format details; see each task's docs).
pub const VARIANTS: usize = 4;
/// Size buckets: < 0.25, < 1, < 4, < 16, < 64 and ≥ 64 megapixels.
pub const BUCKETS: usize = 6;
const LOADS: usize = 2;
const TASKS: usize = Task::ALL.len();
const CELLS: usize = TASKS * MAX_PATHS * VARIANTS * BUCKETS * LOADS;

/// Every background request with this period explores a path not currently chosen.
pub const EXPLORE_EVERY: u32 = 16;
/// A heavy task ([`Task::is_heavy`]) whose chosen path is trusted and at least this many
/// times faster than another explores that slower path only on a [`RECHECK_SECS`] schedule,
/// not every [`EXPLORE_EVERY`] requests: each such run (a whole CPU CLIP pass, say) costs
/// seconds of CPU across every core.
pub const CLEARLY_FASTER: f32 = 2.0;
/// How often a heavy task re-checks a clearly slower path: once per session (the first
/// exploring turn), then at most once in this many seconds. A new fingerprint (other GPU,
/// driver or app version) or "Measure again" starts over.
pub const RECHECK_SECS: u64 = 30 * 60;
/// A background choice "keeps up" when it is at most this many times slower than the fastest.
pub const KEEP_UP: f32 = 4.0;
/// An interactive request on battery takes a cheaper path if it is at most this much slower.
pub const BATTERY_SLACK: f32 = 1.25;
/// Another path must beat the CPU reference by this factor to be chosen (no flapping on noise).
const MARGIN: f32 = 0.97;
/// Weight of new samples in the running averages (the first few are plain means).
const EWMA: f32 = 0.2;
/// The smallest wall rate a measurement stores (µs per megapixel): cells are never 0, and
/// stay above 0 in the cache file (written to 0.1).
const MIN_RATE: f32 = 0.1;

// ---------------------------------------------------------------------------------------------
// What is chosen between

/// Work that has more than one implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Task {
    /// A whole baseline JPEG to Y/Cb/Cr planes: our decoder or a hardware decoder
    /// (`specular-hwdec`). Variant 0.
    JpegDecode,
    /// YCbCr planes to RGB until the pixels are in a GPU texture: converted on the CPU before
    /// upload, or uploaded as planes and converted in the shader (`specular-render`).
    /// Variant: 0 4:2:0, 1 4:2:2, 2 4:4:4, 3 other.
    YcbcrToRgb,
    /// PNG unfiltering (whole decode timed): row by row, or wavefront-parallel column segments
    /// (`specular-png`). Variant: compression ratio, see [`png_variant`].
    PngUnfilter,
    /// Hook: HEIC (HEVC tiles) on our CPU path or a hardware decoder (VA-API etc.). Nothing
    /// registers the hardware path yet.
    HeicDecode,
    /// Local AI inference (DESIGN.md §10) on the CPU, the GPU or an NPU. Heavy
    /// ([`Task::is_heavy`]).
    Inference,
    /// H.264 encoding of the slideshow video export, per frame (pixels: the frame size): our
    /// encoder (`specular-h264enc`) or a hardware encoder (`specular-hwdec::encode`).
    /// Variant 0.
    VideoEncode,
    /// Develop renders (`specular-develop`): on the software adapter (lavapipe, WARP; the
    /// reference path, always there) or the hardware GPU.
    /// Pixels: the source size. Variant 0.
    Develop,
}

impl Task {
    pub const ALL: [Task; 7] = [
        Task::JpegDecode,
        Task::YcbcrToRgb,
        Task::PngUnfilter,
        Task::HeicDecode,
        Task::Inference,
        Task::VideoEncode,
        Task::Develop,
    ];

    /// The engines of this task's paths; index 0 is the CPU reference.
    pub const fn engines(self) -> &'static [Engine] {
        match self {
            Task::JpegDecode | Task::HeicDecode => &[Engine::Cpu, Engine::HwDecoder],
            Task::YcbcrToRgb => &[Engine::Cpu, Engine::Gpu],
            Task::PngUnfilter => &[Engine::Cpu, Engine::CpuParallel],
            Task::Inference => &[Engine::Cpu, Engine::Gpu, Engine::Npu],
            Task::VideoEncode => &[Engine::Cpu, Engine::HwEncoder],
            Task::Develop => &[Engine::Cpu, Engine::Gpu],
        }
    }

    /// Name in timing tables and reports.
    pub const fn key(self) -> &'static str {
        match self {
            Task::JpegDecode => "jpeg_decode",
            Task::YcbcrToRgb => "ycbcr_to_rgb",
            Task::PngUnfilter => "png_unfilter",
            Task::HeicDecode => "heic_decode",
            Task::Inference => "inference",
            Task::VideoEncode => "video_encode",
            Task::Develop => "develop",
        }
    }

    /// Whether one run is expensive enough (a whole model pass) that exploring a clearly
    /// slower path is rationed by time ([`RECHECK_SECS`]) rather than by request count.
    pub const fn is_heavy(self) -> bool {
        matches!(self, Task::Inference)
    }

    /// The path index of `engine` in this task, if it has one.
    pub fn path_of(self, engine: Engine) -> Option<usize> {
        self.engines().iter().position(|&e| e == engine)
    }

    fn from_key(s: &str) -> Option<Task> {
        Task::ALL.into_iter().find(|t| t.key() == s)
    }
}

/// What does the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Engine {
    /// One CPU thread, or our usual threading (the reference).
    Cpu,
    /// A CPU variant that spends more CPU time for less wall time.
    CpuParallel,
    /// Shaders on the GPU.
    Gpu,
    /// A fixed-function decoder (VA-API, Media Foundation, VideoToolbox, NVDEC).
    HwDecoder,
    /// A neural processing unit.
    Npu,
    /// A fixed-function video encoder (VA-API; Media Foundation, VideoToolbox, NVENC later).
    HwEncoder,
}

impl Engine {
    /// Whether "CPU only" excludes it (it is not on the CPU).
    pub const fn is_accelerator(self) -> bool {
        matches!(
            self,
            Engine::Gpu | Engine::HwDecoder | Engine::Npu | Engine::HwEncoder
        )
    }

    pub const fn key(self) -> &'static str {
        match self {
            Engine::Cpu => "cpu",
            Engine::CpuParallel => "cpu_parallel",
            Engine::Gpu => "gpu",
            Engine::HwDecoder => "hw_decoder",
            Engine::Npu => "npu",
            Engine::HwEncoder => "hw_encoder",
        }
    }
}

/// What a request needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Objective {
    /// The image on screen or the next one while flipping: lowest time to pixels.
    #[default]
    Interactive,
    /// Prefetch, thumbnails, indexing: leave the CPU to the user, as long as it keeps up.
    Background,
}

thread_local! {
    static OBJECTIVE: Cell<Objective> = const { Cell::new(Objective::Interactive) };
}

/// The objective of work on this thread: set by the job pool from each job's priority, so
/// decoders need no extra parameter. Interactive on threads outside the pool.
pub fn current_objective() -> Objective {
    OBJECTIVE.with(Cell::get)
}

/// Sets [`current_objective`] for this thread.
pub fn set_current_objective(o: Objective) {
    OBJECTIVE.with(|c| c.set(o));
}

/// Settings → Performance → "Processing".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Mode {
    /// Each machine picks per request from its measurements.
    #[default]
    Automatic,
    /// The GPU or hardware path whenever it exists and is trusted.
    PreferGpu,
    /// Never the GPU, a hardware decoder or an NPU (a reproducible baseline).
    CpuOnly,
}

/// A per-task switch over the automatic choice (`performance.fast_yuv_upload`,
/// `performance.hardware_decode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Override {
    #[default]
    Auto,
    /// The non-reference path whenever it exists and has not failed.
    On,
    /// The CPU reference.
    Off,
}

/// Whether a path may be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Trust {
    /// Not on this machine (no device, no driver, not built).
    #[default]
    Absent,
    /// Exists but not yet checked against the CPU reference.
    Unverified,
    /// Checked (bit-exact, or above the PSNR floor for lossy output).
    Trusted,
    /// Failed its check or errored: not used again with this driver.
    Blocked,
}

impl Trust {
    const fn key(self) -> &'static str {
        match self {
            Trust::Absent => "absent",
            Trust::Unverified => "unverified",
            Trust::Trusted => "trusted",
            Trust::Blocked => "blocked",
        }
    }
    fn from_u8(v: u8) -> Trust {
        match v {
            1 => Trust::Unverified,
            2 => Trust::Trusted,
            3 => Trust::Blocked,
            _ => Trust::Absent,
        }
    }
}

/// The kind of GPU, for the priors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum GpuClass {
    #[default]
    Unknown,
    /// No adapter.
    None,
    /// A software rasteriser (llvmpipe, WARP): really the CPU.
    Software,
    /// Shares memory with the CPU.
    Integrated,
    /// Its own memory, behind PCIe.
    Discrete,
}

impl GpuClass {
    /// Name in reports.
    pub const fn key(self) -> &'static str {
        match self {
            GpuClass::Unknown => "unknown",
            GpuClass::None => "none",
            GpuClass::Software => "software",
            GpuClass::Integrated => "integrated",
            GpuClass::Discrete => "discrete",
        }
    }
    fn from_u8(v: u8) -> GpuClass {
        match v {
            1 => GpuClass::None,
            2 => GpuClass::Software,
            3 => GpuClass::Integrated,
            4 => GpuClass::Discrete,
            _ => GpuClass::Unknown,
        }
    }
    fn from_key(s: &str) -> GpuClass {
        (0..5)
            .map(GpuClass::from_u8)
            .find(|g| g.key() == s)
            .unwrap_or_default()
    }
}

/// One request for a decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub objective: Objective,
    /// Pixels of the work (output size).
    pub pixels: u64,
    /// Task-specific detail (see [`Task`]), below [`VARIANTS`].
    pub variant: u8,
    /// The caller can run the CPU reference beside an unverified path and compare.
    pub can_verify: bool,
}

impl Request {
    /// A request with this thread's [`current_objective`].
    pub fn new(pixels: u64, variant: u8) -> Request {
        Request {
            objective: current_objective(),
            pixels,
            variant,
            can_verify: false,
        }
    }
    pub fn objective(mut self, o: Objective) -> Request {
        self.objective = o;
        self
    }
    pub fn verifying(mut self) -> Request {
        self.can_verify = true;
        self
    }
}

/// Why a path was chosen (for reports and logs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    /// The only usable path.
    Only,
    /// Lowest predicted time.
    Faster,
    /// Lowest predicted CPU time that keeps up.
    Cheaper,
    /// Lowest predicted energy (battery or thermal pressure).
    LowerPower,
    /// A background request trying a path not currently chosen.
    Explore,
    /// The per-task override.
    Forced,
    /// Mode "Prefer GPU".
    PreferGpu,
    /// Mode "CPU only".
    CpuOnly,
    /// A calibration run ([`PathSelector::calibration`]).
    Calibrate,
    /// A reduced-size decode (a JPEG level scaled in the DCT): only the CPU reference decodes
    /// at a reduced size, so the selector was not asked ([`note_scaled`]).
    Scaled,
}

impl Reason {
    pub const fn key(self) -> &'static str {
        match self {
            Reason::Only => "only",
            Reason::Faster => "faster",
            Reason::Cheaper => "cheaper",
            Reason::LowerPower => "lower-power",
            Reason::Explore => "explore",
            Reason::Forced => "forced",
            Reason::PreferGpu => "prefer-gpu",
            Reason::CpuOnly => "cpu-only",
            Reason::Calibrate => "calibrate",
            Reason::Scaled => "scaled",
        }
    }
}

/// Cell coordinates of a request: variant, size bucket and load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub variant: u8,
    pub bucket: u8,
    /// Other tracked work was running on the CPU.
    pub busy: bool,
}

/// A decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Choice {
    pub task: Task,
    /// Index into [`Task::engines`].
    pub path: usize,
    pub engine: Engine,
    pub reason: Reason,
    /// The path is unverified: run the CPU reference too, compare, report the result with
    /// [`PathSelector::set_trust`], and use the reference's output.
    pub verify: bool,
    pub key: Key,
    /// Megapixels of the request.
    pub mpx: f32,
}

impl Choice {
    /// Whether the CPU reference path was chosen.
    pub fn is_reference(&self) -> bool {
        self.path == 0
    }

    /// The CPU reference for the same request (the fallback when the chosen path can't take
    /// it after all), keeping the reason.
    pub fn reference(&self) -> Choice {
        Choice {
            path: 0,
            engine: self.task.engines()[0],
            verify: false,
            ..*self
        }
    }
}

/// Predicted cost of one path for one request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    /// Microseconds until the result is usable (pixels in a GPU texture for render paths).
    pub wall_us: f32,
    /// Microseconds of CPU time.
    pub cpu_us: f32,
    /// Where the numbers come from.
    pub source: Source,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// The built-in guess for this kind of machine.
    Prior,
    /// Learned on this machine (calibration or real runs), maybe from a neighbouring cell.
    Measured,
}

// ---------------------------------------------------------------------------------------------
// Priors: the starting guess per device class

/// Built-in starting guess for one path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prior {
    /// Fixed cost per run, microseconds (also used to split measured runs into setup and rate).
    pub setup_us: f32,
    /// Wall microseconds per megapixel above the setup.
    pub wall_per_mpx: f32,
    /// CPU microseconds per megapixel (all threads).
    pub cpu_per_mpx: f32,
    /// Wall-time factor while the CPU is busy with other decodes.
    pub busy: f32,
    /// Energy weight of the engine's wall time, in busy-CPU-core equivalents (the CPU's own
    /// share is counted by `cpu_per_mpx`).
    pub power: f32,
}

const fn p(setup_us: f32, wall_per_mpx: f32, cpu_per_mpx: f32, busy: f32, power: f32) -> Prior {
    Prior {
        setup_us,
        wall_per_mpx,
        cpu_per_mpx,
        busy,
        power,
    }
}

/// The starting guess for `task`'s `engine` on a machine with `gpu` and `threads` CPU threads,
/// for `variant` and size `bucket`. Numbers are from the development machines (Zen 5 laptop
/// with a Radeon 860M) scaled by class; measurements replace them.
pub fn prior(
    task: Task,
    engine: Engine,
    variant: u8,
    bucket: u8,
    gpu: GpuClass,
    threads: u16,
) -> Prior {
    let t = f32::from(threads.max(1));
    // Our multi-threaded decoders scale to about 12 of 16 threads.
    let par = (t * 0.75).max(1.0);
    let (igpu, dgpu) = (gpu == GpuClass::Integrated, gpu == GpuClass::Discrete);
    match (task, engine) {
        // 50 MP 4:2:0 baseline: 54 ms on 16 threads; 91 ms beside another full decode.
        (Task::JpegDecode, Engine::Cpu) => p(300.0, 13_000.0 / par, 13_500.0, 1.8, 0.0),
        // VA-API on the 860M: 211 ms for 50 MP, half of it reading back from uncached
        // memory on one CPU thread. A discrete GPU's decoder (nvJPEG class) reads back over
        // PCIe at several GB/s.
        (Task::JpegDecode | Task::HeicDecode, Engine::HwDecoder) => match gpu {
            GpuClass::Discrete => p(1_500.0, 900.0, 500.0, 1.0, 0.5),
            GpuClass::Integrated => p(3_000.0, 4_200.0, 2_100.0, 1.1, 0.3),
            _ => p(5_000.0, 8_000.0, 4_000.0, 1.1, 0.3),
        },
        (Task::HeicDecode, _) => p(1_000.0, 120_000.0 / par, 120_000.0, 1.8, 0.0),
        // Conversion on one thread plus a 3 bytes/pixel upload, against a 1.5 bytes/pixel
        // upload and a shader pass.
        (Task::YcbcrToRgb, Engine::Cpu) => p(100.0, 4_000.0, 4_000.0, 1.5, 0.0),
        (Task::YcbcrToRgb, Engine::Gpu) => match gpu {
            GpuClass::Discrete => p(200.0, 700.0, 400.0, 1.0, 4.0),
            GpuClass::Integrated => p(200.0, 1_200.0, 600.0, 1.0, 1.5),
            // A software rasteriser runs the shader on the CPU, slower than our CPU path.
            GpuClass::Software => p(500.0, 20_000.0, 60_000.0, 1.5, 0.0),
            _ => p(300.0, 1_500.0, 700.0, 1.0, 2.0),
        },
        // Row by row: inflate and unfilter on two threads; weakly compressed data inflates
        // more slowly.
        (Task::PngUnfilter, Engine::Cpu) => {
            let wall = [10_000.0, 8_000.0, 6_500.0, 6_000.0][usize::from(variant.min(3))];
            p(200.0, wall, wall * 1.4, 1.5, 0.0)
        }
        // Wavefront: about half the wall time for about 50 % more CPU on large, highly
        // compressed images; 15-35 % slower where inflating is the bottleneck. The old fixed
        // gate (≥ 32 MiB and ≥ 8:1) is where this prior starts to win.
        (Task::PngUnfilter, Engine::CpuParallel) => {
            let serial = [10_000.0, 8_000.0, 6_500.0, 6_000.0][usize::from(variant.min(3))];
            let f = match (variant, bucket) {
                (0 | 1, _) => 1.25,
                (_, 0..=3) => 1.05,
                _ => 0.5,
            };
            p(800.0, serial * f, serial * 1.4 * 1.5, 1.8, 0.0)
        }
        (Task::Inference, Engine::Cpu) => p(5_000.0, 400_000.0 / par, 400_000.0, 1.8, 0.0),
        (Task::Inference, Engine::Gpu) if igpu || dgpu => p(
            20_000.0,
            if dgpu { 10_000.0 } else { 60_000.0 },
            2_000.0,
            1.0,
            if dgpu { 4.0 } else { 1.5 },
        ),
        (Task::Inference, Engine::Npu) => p(10_000.0, 80_000.0, 2_000.0, 1.0, 0.2),
        // Per 1080p frame (2.07 MP): High profile on 16 threads, 29 ms (34 fps) under load.
        (Task::VideoEncode, Engine::Cpu) => p(0.0, 14_000.0 / par * 12.0, 140_000.0, 1.6, 0.0),
        // VCN on the 860M: 5 ms a 1080p frame (190-210 fps under load), part of it
        // interleaving NV12 on the CPU; timed per frame, so the setup is the per-frame
        // submit and sync. Discrete encoders (NVENC class) are faster still.
        (Task::VideoEncode, Engine::HwEncoder) => match gpu {
            GpuClass::Discrete => p(200.0, 1_200.0, 500.0, 1.0, 0.5),
            _ => p(300.0, 2_400.0, 600.0, 1.0, 0.3),
        },
        // A default-edit export: the 860M about 40 ms per megapixel (24 MP ≈ 1 s with the
        // decode); a discrete GPU about twice as fast; the software adapter runs every
        // shader on the CPU, about 50 times slower than the integrated GPU.
        (Task::Develop, Engine::Cpu) => {
            p(50_000.0, 2_000_000.0 / par * 12.0, 2_000_000.0, 1.8, 0.0)
        }
        (Task::Develop, Engine::Gpu) => match gpu {
            GpuClass::Discrete => p(20_000.0, 20_000.0, 5_000.0, 1.1, 3.0),
            GpuClass::Integrated => p(20_000.0, 40_000.0, 8_000.0, 1.1, 1.5),
            // Reported as a GPU but not one: no better than the reference.
            GpuClass::Software => p(50_000.0, 2_500_000.0, 2_500_000.0, 1.8, 0.0),
            _ => p(20_000.0, 60_000.0, 8_000.0, 1.1, 1.5),
        },
        // Anything else (a reference path on an odd machine): neutral.
        _ => p(1_000.0, 10_000.0, 10_000.0, 1.5, 1.0),
    }
}

/// An objective's score of path `i` (lower is better; `None`: not eligible).
type Score<'a> = &'a dyn Fn(usize, &Estimate) -> Option<f32>;

/// The size bucket of `pixels`.
pub fn bucket(pixels: u64) -> u8 {
    match pixels {
        0..262_144 => 0,
        262_144..1_048_576 => 1,
        1_048_576..4_194_304 => 2,
        4_194_304..16_777_216 => 3,
        16_777_216..67_108_864 => 4,
        _ => 5,
    }
}

/// [`Task::PngUnfilter`]'s variant from the unfiltered size and the compressed size:
/// below 4:1 (photos), 4–8:1, 8–32:1, 32:1 and above (screenshots, flat graphics).
pub fn png_variant(raw_bytes: u64, compressed_bytes: u64) -> u8 {
    let c = compressed_bytes.max(1);
    match raw_bytes / c {
        0..4 => 0,
        4..8 => 1,
        8..32 => 2,
        _ => 3,
    }
}

// ---------------------------------------------------------------------------------------------
// The table

/// One cell: running averages for one path, variant, bucket and load.
struct CellData {
    /// Samples recorded (saturating).
    n: AtomicU32,
    /// f32 bits: wall microseconds per megapixel above the setup.
    wall: AtomicU32,
    /// CPU samples recorded.
    cpu_n: AtomicU32,
    /// f32 bits: CPU microseconds per megapixel.
    cpu: AtomicU32,
}

impl CellData {
    const fn new() -> CellData {
        CellData {
            n: AtomicU32::new(0),
            wall: AtomicU32::new(0),
            cpu_n: AtomicU32::new(0),
            cpu: AtomicU32::new(0),
        }
    }
    fn reset(&self) {
        for a in [&self.n, &self.wall, &self.cpu_n, &self.cpu] {
            a.store(0, Ordering::Relaxed);
        }
    }
}

/// The prior's wall rate (µs per megapixel above the setup) under the load `busy`.
fn prior_rate(pr: &Prior, busy: bool) -> f32 {
    pr.wall_per_mpx * if busy { pr.busy } else { 1.0 }
}

/// The wall rate a run of `wall_us` for `mpx` megapixels stands for (see
/// [`PathSelector::record`]): never below [`MIN_RATE`].
fn wall_rate(pr: &Prior, busy: bool, mpx: f32, wall_us: f32) -> f32 {
    let rate = prior_rate(pr, busy) * mpx;
    let total = pr.setup_us + rate;
    let share = if total > 0.0 { rate / total } else { 1.0 };
    ((wall_us - pr.setup_us).max(wall_us * share) / mpx).max(MIN_RATE)
}

/// The fraction of the prior's setup a cell with wall rate `rate` implies: 1 unless the
/// path measured faster than its prior (see [`PathSelector::record`]).
fn setup_scale(pr: &Prior, busy: bool, rate: f32) -> f32 {
    let prior = prior_rate(pr, busy);
    if prior > 0.0 && rate.is_finite() {
        (rate / prior).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn fold(avg: &AtomicU32, n: &AtomicU32, sample: f32) {
    // Concurrent records may lose one update; the averages are statistics, not counters.
    let k = n.load(Ordering::Relaxed);
    let old = f32::from_bits(avg.load(Ordering::Relaxed));
    let w = if k < 5 { 1.0 / (k as f32 + 1.0) } else { EWMA };
    let new = old + (sample - old) * w;
    avg.store(new.to_bits(), Ordering::Relaxed);
    n.store(k.saturating_add(1), Ordering::Relaxed);
}

fn cell_index(task: Task, path: usize, key: Key) -> usize {
    let v = usize::from(key.variant).min(VARIANTS - 1);
    let b = usize::from(key.bucket).min(BUCKETS - 1);
    ((((task as usize * MAX_PATHS + path) * VARIANTS + v) * BUCKETS + b) * LOADS)
        + usize::from(key.busy)
}

/// The per-machine path selector. One per process ([`selector`]); tests make their own.
pub struct PathSelector {
    cells: [CellData; CELLS],
    trust: [AtomicU8; TASKS * MAX_PATHS],
    overrides: [AtomicU8; TASKS],
    /// Background requests per task, for exploration.
    background: [AtomicU32; TASKS],
    mode: AtomicU8,
    power_saving: AtomicBool,
    gpu: AtomicU8,
    threads: AtomicU16,
    /// Tracked runs in flight (the load signal).
    inflight: AtomicUsize,
    calibrated: AtomicBool,
    /// Records since the last save.
    dirty: AtomicU32,
    fingerprint: Mutex<String>,
    // -- for the snapshot (snapshot.rs) --
    /// When and why each blocked path was blocked: unix seconds << 8 | [`Failure`].
    blocks: [AtomicU64; TASKS * MAX_PATHS],
    /// Exploring runs in flight, per task.
    exploring: [AtomicU32; TASKS],
    /// Per heavy task: when it last explored a clearly slower path, in [`session_secs`] + 1
    /// (0: not this session).
    rechecked: [AtomicU64; TASKS],
}

/// Seconds since this process first asked (the clock for [`RECHECK_SECS`]).
fn session_secs() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs()
}

impl Default for PathSelector {
    fn default() -> Self {
        PathSelector::new()
    }
}

static GLOBAL: PathSelector = PathSelector::new();

/// The process's selector.
pub fn selector() -> &'static PathSelector {
    &GLOBAL
}

impl PathSelector {
    /// A selector with only the CPU reference paths, priors for an unknown machine.
    pub const fn new() -> PathSelector {
        PathSelector {
            cells: [const { CellData::new() }; CELLS],
            trust: [const { AtomicU8::new(0) }; TASKS * MAX_PATHS],
            overrides: [const { AtomicU8::new(0) }; TASKS],
            background: [const { AtomicU32::new(0) }; TASKS],
            mode: AtomicU8::new(0),
            power_saving: AtomicBool::new(false),
            gpu: AtomicU8::new(0),
            threads: AtomicU16::new(8),
            inflight: AtomicUsize::new(0),
            calibrated: AtomicBool::new(false),
            dirty: AtomicU32::new(0),
            fingerprint: Mutex::new(String::new()),
            blocks: [const { AtomicU64::new(0) }; TASKS * MAX_PATHS],
            exploring: [const { AtomicU32::new(0) }; TASKS],
            rechecked: [const { AtomicU64::new(0) }; TASKS],
        }
    }

    // -- probe and settings ---------------------------------------------------------------

    /// Records the device class the priors are taken for.
    pub fn set_machine(&self, gpu: GpuClass, threads: usize) {
        self.gpu.store(gpu as u8, Ordering::Relaxed);
        self.threads.store(
            threads.clamp(1, u16::MAX as usize) as u16,
            Ordering::Relaxed,
        );
    }

    pub fn gpu_class(&self) -> GpuClass {
        GpuClass::from_u8(self.gpu.load(Ordering::Relaxed))
    }

    pub fn threads(&self) -> u16 {
        self.threads.load(Ordering::Relaxed)
    }

    pub fn set_mode(&self, m: Mode) {
        self.mode.store(m as u8, Ordering::Relaxed);
    }

    pub fn mode(&self) -> Mode {
        match self.mode.load(Ordering::Relaxed) {
            1 => Mode::PreferGpu,
            2 => Mode::CpuOnly,
            _ => Mode::Automatic,
        }
    }

    pub fn set_override(&self, task: Task, o: Override) {
        self.overrides[task as usize].store(o as u8, Ordering::Relaxed);
    }

    pub fn override_of(&self, task: Task) -> Override {
        match self.overrides[task as usize].load(Ordering::Relaxed) {
            1 => Override::On,
            2 => Override::Off,
            _ => Override::Auto,
        }
    }

    /// On battery, in a low-power mode or under thermal pressure.
    pub fn set_power_saving(&self, on: bool) {
        self.power_saving.store(on, Ordering::Relaxed);
    }

    pub fn power_saving(&self) -> bool {
        self.power_saving.load(Ordering::Relaxed)
    }

    /// Whether a path exists and may be used. The CPU reference (path 0) is always
    /// trusted and cannot be changed. A blocked path stays blocked unless `force` (a new
    /// fingerprint resets everything anyway).
    pub fn set_trust(&self, task: Task, engine: Engine, t: Trust) {
        let Some(i) = task.path_of(engine).filter(|&i| i > 0) else {
            return;
        };
        let a = &self.trust[task as usize * MAX_PATHS + i];
        // Blocked is sticky: a later "exists" from the probe must not undo it.
        let _ = a.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
            (Trust::from_u8(old) != Trust::Blocked || t == Trust::Absent).then_some(t as u8)
        });
        if t == Trust::Blocked {
            self.dirty.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Marks a path that exists as unverified, unless it is already trusted or blocked.
    pub fn set_available(&self, task: Task, engine: Engine) {
        let Some(i) = task.path_of(engine).filter(|&i| i > 0) else {
            return;
        };
        let _ = self.trust[task as usize * MAX_PATHS + i].compare_exchange(
            Trust::Absent as u8,
            Trust::Unverified as u8,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    /// Records the outcome of a correctness check of `engine` against the CPU reference.
    pub fn verified(&self, task: Task, engine: Engine, ok: bool) {
        self.set_trust(
            task,
            engine,
            if ok { Trust::Trusted } else { Trust::Blocked },
        );
        if !ok {
            self.note_block(task, engine, Failure::CheckFailed, None);
            crate::paths::log_block(task, engine, "failed its check against the CPU reference");
        }
    }

    /// Blocklists a path after an error, device loss or driver crash.
    pub fn block(&self, task: Task, engine: Engine, why: &str) {
        self.set_trust(task, engine, Trust::Blocked);
        self.note_block(task, engine, Failure::classify(why), None);
        log_block(task, engine, why);
    }

    pub fn trust(&self, task: Task, path: usize) -> Trust {
        if path == 0 {
            return Trust::Trusted;
        }
        if path >= task.engines().len() {
            return Trust::Absent;
        }
        Trust::from_u8(self.trust[task as usize * MAX_PATHS + path].load(Ordering::Relaxed))
    }

    /// Whether the calibration ran for this fingerprint.
    pub fn calibrated(&self) -> bool {
        self.calibrated.load(Ordering::Relaxed)
    }

    pub fn set_calibrated(&self) {
        self.calibrated.store(true, Ordering::Relaxed);
        self.dirty.fetch_add(1, Ordering::Relaxed);
    }

    /// Records since the table was last saved (and resets the count).
    pub fn take_dirty(&self) -> u32 {
        self.dirty.swap(0, Ordering::Relaxed)
    }

    pub fn fingerprint(&self) -> String {
        self.fingerprint.lock().clone()
    }

    /// Sets the machine fingerprint. A different one than before forgets everything
    /// learned (timings, trust, blocklist): it is a different GPU, driver, CPU or app.
    pub fn set_fingerprint(&self, f: &str) {
        let mut g = self.fingerprint.lock();
        if *g != f {
            if !g.is_empty() {
                self.forget();
            }
            *g = f.to_owned();
        }
    }

    /// Forgets timings, trust and calibration (keeps mode, overrides and machine class).
    pub fn forget(&self) {
        for c in &self.cells {
            c.reset();
        }
        for t in &self.trust {
            t.store(0, Ordering::Relaxed);
        }
        for b in &self.blocks {
            b.store(0, Ordering::Relaxed);
        }
        for r in &self.rechecked {
            r.store(0, Ordering::Relaxed);
        }
        self.calibrated.store(false, Ordering::Relaxed);
    }

    // -- load ------------------------------------------------------------------------------

    /// Tracked runs in flight.
    pub fn inflight(&self) -> usize {
        self.inflight.load(Ordering::Relaxed)
    }

    /// Counts as CPU load for decisions while alive (for heavy work that is not itself
    /// a [`Run`], e.g. other decoders).
    pub fn busy(&'static self) -> Busy {
        self.inflight.fetch_add(1, Ordering::Relaxed);
        // Untracked work takes its share of the CPU time of tracked work it overlaps (as a
        // decode on all threads would), so that runs are not charged for it.
        let par = (f32::from(self.threads()) * 0.75).max(1.0);
        Busy(&self.inflight, cpu::open(par - 1.0))
    }

    // -- predictions -------------------------------------------------------------------------

    fn prior_of(&self, task: Task, path: usize, key: Key) -> Prior {
        prior(
            task,
            task.engines()[path],
            key.variant,
            key.bucket,
            self.gpu_class(),
            self.threads(),
        )
    }

    /// Predicted cost of `path` for a request of `mpx` megapixels with `key`.
    pub fn estimate(&self, task: Task, path: usize, key: Key, mpx: f32) -> Estimate {
        let pr = self.prior_of(task, path, key);
        let at = |k: Key| &self.cells[cell_index(task, path, k)];
        // The exact cell, the same cell under the other load (scaled), then the nearest
        // bucket under this load: at most 2 + 2 × (BUCKETS - 1) lookups.
        let mut found: Option<(&CellData, Key, f32)> = None;
        let c = at(key);
        if c.n.load(Ordering::Relaxed) > 0 {
            found = Some((c, key, 1.0));
        } else {
            let k = Key {
                busy: !key.busy,
                ..key
            };
            let other = at(k);
            if other.n.load(Ordering::Relaxed) > 0 {
                found = Some((other, k, if key.busy { pr.busy } else { 1.0 / pr.busy }));
            } else {
                for d in 1..BUCKETS as i32 {
                    for b in [i32::from(key.bucket) - d, i32::from(key.bucket) + d] {
                        if (0..BUCKETS as i32).contains(&b) {
                            let k = Key {
                                bucket: b as u8,
                                ..key
                            };
                            let c = at(k);
                            if c.n.load(Ordering::Relaxed) > 0 {
                                found = Some((c, k, 1.0));
                                break;
                            }
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
            }
        }
        match found {
            Some((c, at_key, scale)) => {
                let raw = f32::from_bits(c.wall.load(Ordering::Relaxed));
                // Faster than the prior: the setup is faster too (see `record`).
                let setup =
                    pr.setup_us * setup_scale(&self.prior_of(task, path, at_key), at_key.busy, raw);
                let wall = raw * scale;
                let wall_us = setup + wall * mpx;
                let cpu_us = if c.cpu_n.load(Ordering::Relaxed) > 0 {
                    f32::from_bits(c.cpu.load(Ordering::Relaxed)) * mpx
                } else {
                    // Unmeasured CPU time: the prior's ratio of CPU to wall time.
                    let prior_wall = pr.setup_us + pr.wall_per_mpx * mpx;
                    (pr.cpu_per_mpx * mpx) * (wall_us / prior_wall.max(1.0))
                };
                Estimate {
                    wall_us,
                    cpu_us,
                    source: Source::Measured,
                }
            }
            None => {
                let busy = if key.busy { pr.busy } else { 1.0 };
                Estimate {
                    wall_us: pr.setup_us + pr.wall_per_mpx * busy * mpx,
                    cpu_us: pr.cpu_per_mpx * mpx,
                    source: Source::Prior,
                }
            }
        }
    }

    fn energy(&self, task: Task, path: usize, key: Key, e: &Estimate) -> f32 {
        e.cpu_us + self.prior_of(task, path, key).power * e.wall_us
    }

    /// The key of `req` under the current load.
    pub fn key_of(&self, req: &Request) -> Key {
        Key {
            variant: req.variant.min(VARIANTS as u8 - 1),
            bucket: bucket(req.pixels),
            busy: self.inflight() > 0,
        }
    }

    // -- the decision ----------------------------------------------------------------------

    /// Picks the path for `req`. O(1), no allocation. The process's selector also leaves the
    /// choice in this thread's [`Note`] (see [`take_note`]).
    pub fn choose(&self, task: Task, req: &Request) -> Choice {
        let c = self.decide(task, req, true);
        if std::ptr::eq(self, &GLOBAL) {
            NOTE.with(|n| {
                n.set(Some(Note {
                    choice: c,
                    fallback: None,
                    checked: false,
                    alt_trust: self.trust(task, 1),
                    scaled: None,
                }))
            });
        }
        c
    }

    /// What [`choose`](Self::choose) picks for `req` when not exploring, without moving the
    /// exploration counter or leaving a [`Note`]: for pages that say where work runs.
    pub fn preview(&self, task: Task, req: &Request) -> Choice {
        self.decide(task, req, false)
    }

    fn decide(&self, task: Task, req: &Request, explore: bool) -> Choice {
        let key = self.key_of(req);
        let mpx = req.pixels as f32 / 1e6;
        let engines = task.engines();
        let mode = self.mode();
        let ov = self.override_of(task);
        let make = |path: usize, reason: Reason| Choice {
            task,
            path,
            engine: engines[path],
            reason,
            verify: self.trust(task, path) == Trust::Unverified,
            key,
            mpx,
        };
        let reference = |reason| make(0, reason);
        // Usable: trusted, or unverified when the caller can check it.
        let usable = |i: usize| {
            let allowed = !(mode == Mode::CpuOnly && engines[i].is_accelerator());
            allowed
                && match self.trust(task, i) {
                    Trust::Trusted => true,
                    Trust::Unverified => req.can_verify,
                    Trust::Absent | Trust::Blocked => false,
                }
        };
        if ov == Override::Off {
            return reference(Reason::Forced);
        }
        let n = engines.len();
        let any_alt = (1..n).any(usable);
        if !any_alt {
            let cpu_only = mode == Mode::CpuOnly && (1..n).any(|i| engines[i].is_accelerator());
            return reference(if cpu_only {
                Reason::CpuOnly
            } else {
                Reason::Only
            });
        }
        if ov == Override::On {
            let i = (1..n).find(|&i| usable(i)).unwrap_or(0);
            return make(i, Reason::Forced);
        }
        if mode == Mode::PreferGpu
            && let Some(i) = (1..n).find(|&i| usable(i) && engines[i].is_accelerator())
        {
            return make(i, Reason::PreferGpu);
        }
        // Automatic. An unverified path is only tried by background requests (exploring),
        // never while the user waits.
        let mut est = [None::<Estimate>; MAX_PATHS];
        for (i, e) in est.iter_mut().enumerate().take(n) {
            if usable(i) && (i == 0 || self.trust(task, i) == Trust::Trusted) {
                *e = Some(self.estimate(task, i, key, mpx));
            }
        }
        let best_wall = est
            .iter()
            .flatten()
            .map(|e| e.wall_us)
            .fold(f32::INFINITY, f32::min);
        let power = self.power_saving();
        let (reason, score): (Reason, Score<'_>) = match (req.objective, power) {
            (Objective::Interactive, false) => (Reason::Faster, &|_, e| Some(e.wall_us)),
            (Objective::Interactive, true) => (Reason::LowerPower, &|i, e| {
                (e.wall_us <= best_wall * BATTERY_SLACK).then(|| self.energy(task, i, key, e))
            }),
            (Objective::Background, false) => (Reason::Cheaper, &|_, e| {
                (e.wall_us <= best_wall * KEEP_UP).then_some(e.cpu_us)
            }),
            (Objective::Background, true) => (Reason::LowerPower, &|i, e| {
                (e.wall_us <= best_wall * KEEP_UP).then(|| self.energy(task, i, key, e))
            }),
        };
        let mut best = 0usize;
        let mut best_score = est[0]
            .as_ref()
            .and_then(|e| score(0, e))
            .unwrap_or(f32::INFINITY);
        let ref_score = best_score;
        for (i, e) in est.iter().enumerate().take(n).skip(1) {
            if let Some(s) = e.as_ref().and_then(|e| score(i, e))
                && s < best_score
                && s < ref_score * MARGIN
            {
                best = i;
                best_score = s;
            }
        }
        if explore && req.objective == Objective::Background {
            let k = self.background[task as usize].fetch_add(1, Ordering::Relaxed);
            if k % EXPLORE_EVERY == EXPLORE_EVERY - 1
                && let Some(i) = self.explore_target(task, key, best, &est, req, mpx)
                && self.recheck_due(task, best, i, &est, session_secs())
            {
                return make(i, Reason::Explore);
            }
        }
        make(
            best,
            if best == 0 && est[1..].iter().all(Option::is_none) {
                Reason::Only
            } else {
                reason
            },
        )
    }

    /// The path a background request explores instead of `best`: the usable one with the
    /// fewest samples for this key, unless its predicted time is far off.
    fn explore_target(
        &self,
        task: Task,
        key: Key,
        best: usize,
        est: &[Option<Estimate>; MAX_PATHS],
        req: &Request,
        mpx: f32,
    ) -> Option<usize> {
        let n = task.engines().len();
        let best_wall = est[best].map_or(f32::INFINITY, |e| e.wall_us);
        let mut pick: Option<(usize, u32)> = None;
        for i in (0..n).filter(|&i| i != best) {
            let ok = match self.trust(task, i) {
                Trust::Trusted => true,
                Trust::Unverified => req.can_verify,
                _ => false,
            } && !(self.mode() == Mode::CpuOnly && task.engines()[i].is_accelerator());
            if !ok {
                continue;
            }
            let e = est[i].unwrap_or_else(|| self.estimate(task, i, key, mpx));
            if e.wall_us > best_wall * KEEP_UP * 2.0 {
                continue;
            }
            let samples = self.cells[cell_index(task, i, key)]
                .n
                .load(Ordering::Relaxed);
            if pick.is_none_or(|(_, s)| samples < s) {
                pick = Some((i, samples));
            }
        }
        pick.map(|p| p.0)
    }

    /// Whether exploring path `i` instead of `best` may go ahead now (`now`: [`session_secs`]).
    /// Always, except for a heavy task whose `best` is a trusted accelerator at least
    /// [`CLEARLY_FASTER`] times faster than `i`: then once per session and at most every
    /// [`RECHECK_SECS`] (a turn that falls inside the window serves `best` instead). A
    /// regressed `best` still gets caught: its own runs are timed, and once it stops being
    /// clearly faster every exploring turn checks `i` again.
    fn recheck_due(
        &self,
        task: Task,
        best: usize,
        i: usize,
        est: &[Option<Estimate>; MAX_PATHS],
        now: u64,
    ) -> bool {
        if !task.is_heavy()
            || best == 0
            || self.trust(task, best) != Trust::Trusted
            || !task.engines()[best].is_accelerator()
        {
            return true;
        }
        let (Some(b), Some(e)) = (est[best], est[i]) else {
            return true;
        };
        if b.wall_us * CLEARLY_FASTER > e.wall_us {
            return true;
        }
        let r = &self.rechecked[task as usize];
        let last = r.load(Ordering::Relaxed);
        if last != 0 && now + 1 < last + RECHECK_SECS {
            return false;
        }
        r.compare_exchange(last, now + 1, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }

    // -- recording -------------------------------------------------------------------------

    /// Records one run of `path` for `key`: wall and (if measured) CPU microseconds.
    ///
    /// The wall time is split into the prior's setup and a rate per megapixel. A run at
    /// least as slow as the prior predicts keeps the whole setup: the rate is what is left.
    /// A faster run is taken to be faster at both in the same proportion (the rate is its
    /// share of the run by the prior), and [`estimate`](Self::estimate) scales the setup
    /// down by the same factor; either way the estimate at the measured size is the measured
    /// time, and no measurement stores a rate of 0 (a run shorter than the prior's setup
    /// alone used to).
    pub fn record(
        &self,
        task: Task,
        path: usize,
        key: Key,
        mpx: f32,
        wall_us: f32,
        cpu_us: Option<f32>,
    ) {
        if path >= task.engines().len() || !wall_us.is_finite() || wall_us < 0.0 {
            return;
        }
        let mpx = mpx.max(0.01);
        let pr = self.prior_of(task, path, key);
        let c = &self.cells[cell_index(task, path, key)];
        fold(&c.wall, &c.n, wall_rate(&pr, key.busy, mpx, wall_us));
        if let Some(cpu) = cpu_us.filter(|c| c.is_finite() && *c >= 0.0) {
            fold(&c.cpu, &c.cpu_n, cpu / mpx);
        }
        self.dirty.fetch_add(1, Ordering::Relaxed);
    }

    /// Records only the CPU time of a run whose wall time can't be measured without
    /// waiting for it (GPU work in flight); the wall times come from calibration.
    pub fn record_cpu(&self, task: Task, path: usize, key: Key, mpx: f32, cpu_us: f32) {
        if path >= task.engines().len() || !cpu_us.is_finite() || cpu_us < 0.0 {
            return;
        }
        let c = &self.cells[cell_index(task, path, key)];
        fold(&c.cpu, &c.cpu_n, cpu_us / mpx.max(0.01));
        self.dirty.fetch_add(1, Ordering::Relaxed);
    }

    /// Starts timing a run of `choice` (counts as load until finished or dropped). Its CPU
    /// time is counted over every thread of the process from here to
    /// [`finish`](Run::finish), shared with other tracked work that overlaps it (see
    /// `paths/cpu.rs`); start and finish it on the thread that does or waits for the work.
    pub fn start(&self, choice: &Choice) -> Run<'_> {
        self.inflight.fetch_add(1, Ordering::Relaxed);
        let path = choice.path.min(choice.task.engines().len() - 1);
        let pr = self.prior_of(choice.task, path, choice.key);
        let exploring = choice.reason == Reason::Explore;
        if exploring {
            self.exploring[choice.task as usize].fetch_add(1, Ordering::Relaxed);
        }
        Run {
            sel: self,
            task: choice.task,
            path: choice.path,
            key: choice.key,
            mpx: choice.mpx,
            cpu: cpu::open(cpu::weight(pr.cpu_per_mpx, pr.wall_per_mpx)),
            t0: Instant::now(),
            done: false,
            exploring,
        }
    }

    /// A run of `path` on built-in input for calibration (not a decision: no mode,
    /// override or exploration). Time it with [`start`](Self::start) while nothing else runs.
    pub fn calibration(&self, task: Task, path: usize, pixels: u64, variant: u8) -> Choice {
        let path = path.min(task.engines().len() - 1);
        Choice {
            task,
            path,
            engine: task.engines()[path],
            reason: Reason::Calibrate,
            verify: false,
            key: Key {
                variant: variant.min(VARIANTS as u8 - 1),
                bucket: bucket(pixels),
                busy: false,
            },
            mpx: pixels as f32 / 1e6,
        }
    }

    /// Samples recorded for a cell (reports and tests).
    pub fn samples(&self, task: Task, path: usize, key: Key) -> u32 {
        self.cells[cell_index(task, path, key)]
            .n
            .load(Ordering::Relaxed)
    }

    // -- timing tables ---------------------------------------------------------------------

    /// The whole state as text: the cache file's format and the replay format of tests.
    pub fn to_table(&self) -> String {
        let mut s = String::from("# Specular path timings (DESIGN.md §12.3.1), version 1\n");
        let _ = writeln!(s, "fingerprint {}", self.fingerprint.lock());
        let _ = writeln!(
            s,
            "machine gpu={} threads={}",
            self.gpu_class().key(),
            self.threads()
        );
        let _ = writeln!(s, "calibrated {}", self.calibrated());
        for task in Task::ALL {
            for (i, e) in task.engines().iter().enumerate().skip(1) {
                let t = self.trust(task, i);
                if t != Trust::Absent {
                    let _ = writeln!(s, "trust {} {} {}", task.key(), e.key(), t.key());
                }
            }
        }
        self.write_blocks(&mut s);
        s.push_str("# cell task engine variant bucket load samples wall_us_per_mpx cpu_samples cpu_us_per_mpx\n");
        for task in Task::ALL {
            for (i, e) in task.engines().iter().enumerate() {
                for v in 0..VARIANTS as u8 {
                    for b in 0..BUCKETS as u8 {
                        for busy in [false, true] {
                            let key = Key {
                                variant: v,
                                bucket: b,
                                busy,
                            };
                            let c = &self.cells[cell_index(task, i, key)];
                            let n = c.n.load(Ordering::Relaxed);
                            if n == 0 && c.cpu_n.load(Ordering::Relaxed) == 0 {
                                continue;
                            }
                            let _ = writeln!(
                                s,
                                "cell {} {} {v} {b} {} {n} {:.1} {} {:.1}",
                                task.key(),
                                e.key(),
                                if busy { "busy" } else { "idle" },
                                f32::from_bits(c.wall.load(Ordering::Relaxed)),
                                c.cpu_n.load(Ordering::Relaxed),
                                f32::from_bits(c.cpu.load(Ordering::Relaxed)),
                            );
                        }
                    }
                }
            }
        }
        s
    }

    /// Loads a table written by [`to_table`](Self::to_table) (or by hand for tests). Lines it
    /// doesn't understand are skipped. With `expect_fingerprint`, a table for another machine
    /// is ignored and `false` returned.
    pub fn load_table(&self, text: &str, expect_fingerprint: Option<&str>) -> bool {
        let fp = text
            .lines()
            .find_map(|l| l.strip_prefix("fingerprint "))
            .map(str::trim)
            .unwrap_or("");
        if expect_fingerprint.is_some_and(|f| f != fp) {
            return false;
        }
        self.forget();
        *self.fingerprint.lock() = fp.to_owned();
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            match w.as_slice() {
                ["machine", rest @ ..] => {
                    for kv in rest {
                        match kv.split_once('=') {
                            Some(("gpu", g)) => self
                                .gpu
                                .store(GpuClass::from_key(g) as u8, Ordering::Relaxed),
                            Some(("threads", t)) => {
                                if let Ok(t) = t.parse() {
                                    self.threads.store(t, Ordering::Relaxed);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ["calibrated", v] => self.calibrated.store(*v == "true", Ordering::Relaxed),
                ["trust", task, engine, t] => {
                    let Some(task) = Task::from_key(task) else {
                        continue;
                    };
                    let Some(i) = engine_index(task, engine) else {
                        continue;
                    };
                    let t = (0..4)
                        .map(Trust::from_u8)
                        .find(|x| x.key() == *t)
                        .unwrap_or_default();
                    if i > 0 {
                        self.trust[task as usize * MAX_PATHS + i].store(t as u8, Ordering::Relaxed);
                    }
                }
                ["blocked", task, engine, failure, at] => {
                    self.read_block(task, engine, failure, at)
                }
                ["cell", task, engine, v, b, load, n, wall, cpu_n, cpu] => {
                    let Some(task) = Task::from_key(task) else {
                        continue;
                    };
                    let Some(i) = engine_index(task, engine) else {
                        continue;
                    };
                    let (Ok(v), Ok(b), Ok(n), Ok(wall), Ok(cpu_n), Ok(cpu)) = (
                        v.parse::<u8>(),
                        b.parse::<u8>(),
                        n.parse::<u32>(),
                        wall.parse::<f32>(),
                        cpu_n.parse::<u32>(),
                        cpu.parse::<f32>(),
                    ) else {
                        continue;
                    };
                    // A wall rate of 0 (written before measurements were kept above 0) or
                    // garbage is not a measurement.
                    if usize::from(v) >= VARIANTS
                        || usize::from(b) >= BUCKETS
                        || !(wall.is_finite() && wall > 0.0)
                    {
                        continue;
                    }
                    let key = Key {
                        variant: v,
                        bucket: b,
                        busy: *load == "busy",
                    };
                    let c = &self.cells[cell_index(task, i, key)];
                    c.n.store(n, Ordering::Relaxed);
                    c.wall.store(wall.to_bits(), Ordering::Relaxed);
                    c.cpu_n.store(cpu_n, Ordering::Relaxed);
                    c.cpu.store(cpu.to_bits(), Ordering::Relaxed);
                }
                _ => {}
            }
        }
        true
    }

    /// Loads the cache file if it was written for `fingerprint`; otherwise sets the
    /// fingerprint and starts afresh. Returns whether anything was loaded.
    pub fn load_file(&self, path: &std::path::Path, fingerprint: &str) -> bool {
        let loaded = std::fs::read_to_string(path)
            .ok()
            .is_some_and(|t| self.load_table(&t, Some(fingerprint)));
        if !loaded {
            self.set_fingerprint(fingerprint);
        }
        loaded
    }

    /// Writes the table atomically (temporary file, then rename).
    pub fn save_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_table())?;
        std::fs::rename(&tmp, path)
    }

    // -- reports ---------------------------------------------------------------------------

    /// What each task would do now for a `mpx`-megapixel request, interactive and in the
    /// background, with each path's predicted cost (the hardware report). Tasks with a
    /// [`Unit`] of their own are reported at that size ([`Choice::mpx`] says which).
    pub fn report(&self, mpx: f32) -> Vec<TaskReport> {
        Task::ALL
            .into_iter()
            .map(|task| {
                let pixels = task.unit().pixels().unwrap_or((mpx * 1e6) as u64);
                let mpx = pixels as f32 / 1e6;
                let key = Key {
                    variant: 0,
                    bucket: bucket(pixels),
                    busy: false,
                };
                let paths = task
                    .engines()
                    .iter()
                    .enumerate()
                    .map(|(i, &engine)| PathReport {
                        engine,
                        trust: self.trust(task, i),
                        estimate: self.estimate(task, i, key, mpx),
                        samples: (0..VARIANTS as u8)
                            .flat_map(|v| (0..BUCKETS as u8).map(move |b| (v, b)))
                            .flat_map(|(v, b)| [(v, b, false), (v, b, true)])
                            .map(|(variant, bucket, busy)| {
                                self.samples(
                                    task,
                                    i,
                                    Key {
                                        variant,
                                        bucket,
                                        busy,
                                    },
                                )
                            })
                            .sum(),
                    })
                    .collect();
                // Reports must not move the exploration counters.
                let probe = PathSelector::new();
                probe.copy_from(self);
                let req = Request {
                    objective: Objective::Interactive,
                    pixels,
                    variant: 0,
                    can_verify: false,
                };
                TaskReport {
                    task,
                    paths,
                    interactive: probe.choose(task, &req),
                    background: probe.choose(
                        task,
                        &Request {
                            objective: Objective::Background,
                            ..req
                        },
                    ),
                }
            })
            .collect()
    }

    fn copy_from(&self, o: &PathSelector) {
        for (a, b) in self.cells.iter().zip(&o.cells) {
            for (x, y) in [
                (&a.n, &b.n),
                (&a.wall, &b.wall),
                (&a.cpu_n, &b.cpu_n),
                (&a.cpu, &b.cpu),
            ] {
                x.store(y.load(Ordering::Relaxed), Ordering::Relaxed);
            }
        }
        for (x, y) in self.trust.iter().zip(&o.trust) {
            x.store(y.load(Ordering::Relaxed), Ordering::Relaxed);
        }
        for (x, y) in self.overrides.iter().zip(&o.overrides) {
            x.store(y.load(Ordering::Relaxed), Ordering::Relaxed);
        }
        self.mode
            .store(o.mode.load(Ordering::Relaxed), Ordering::Relaxed);
        self.power_saving.store(o.power_saving(), Ordering::Relaxed);
        self.gpu
            .store(o.gpu.load(Ordering::Relaxed), Ordering::Relaxed);
        self.threads.store(o.threads(), Ordering::Relaxed);
    }

    /// The report as plain lines (`--hw-report`, logs).
    pub fn report_lines(&self, mpx: f32) -> Vec<String> {
        let mut v = vec![format!(
            "mode {:?}, machine {} GPU / {} threads, {}{}",
            self.mode(),
            self.gpu_class().key(),
            self.threads(),
            if self.calibrated() {
                "calibrated"
            } else {
                "not calibrated yet"
            },
            if self.power_saving() {
                ", power saving"
            } else {
                ""
            },
        )];
        for r in self.report(mpx) {
            if r.paths.iter().skip(1).all(|p| p.trust == Trust::Absent) {
                continue;
            }
            let paths: Vec<String> = r
                .paths
                .iter()
                .map(|p| {
                    format!(
                        "{} {:.1} ms wall / {:.1} ms CPU ({}{}, {} runs)",
                        p.engine.key(),
                        p.estimate.wall_us / 1e3,
                        p.estimate.cpu_us / 1e3,
                        match p.estimate.source {
                            Source::Prior => "guess",
                            Source::Measured => "measured",
                        },
                        if p.trust == Trust::Trusted {
                            ""
                        } else {
                            p.trust_suffix()
                        },
                        p.samples
                    )
                })
                .collect();
            v.push(format!(
                "{} at {} MP: {}; interactive → {} ({}), background → {} ({})",
                r.task.key(),
                r.task.unit().mpx(mpx),
                paths.join(", "),
                r.interactive.engine.key(),
                r.interactive.reason.key(),
                r.background.engine.key(),
                r.background.reason.key(),
            ));
        }
        v
    }
}

/// Reported state of one path.
#[derive(Clone, Debug)]
pub struct PathReport {
    pub engine: Engine,
    pub trust: Trust,
    pub estimate: Estimate,
    pub samples: u32,
}

impl PathReport {
    fn trust_suffix(&self) -> &'static str {
        match self.trust {
            Trust::Absent => ", absent",
            Trust::Unverified => ", unverified",
            Trust::Blocked => ", blocked",
            Trust::Trusted => "",
        }
    }
}

/// Reported state of one task.
#[derive(Clone, Debug)]
pub struct TaskReport {
    pub task: Task,
    pub paths: Vec<PathReport>,
    pub interactive: Choice,
    pub background: Choice,
}

fn engine_index(task: Task, key: &str) -> Option<usize> {
    task.engines().iter().position(|e| e.key() == key)
}

fn log_block(task: Task, engine: Engine, why: &str) {
    specular_log::warn!(target: specular_log::DECODE,
        "{} on {}: {why}; using the CPU path with this driver", task.key(), engine.key());
}

/// Holds one unit of CPU load (see [`PathSelector::busy`]).
pub struct Busy(&'static AtomicUsize, Option<u64>);

impl Drop for Busy {
    fn drop(&mut self) {
        if let Some(id) = self.1 {
            cpu::close(id);
        }
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl std::fmt::Debug for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Busy")
    }
}

/// One timed run (see [`PathSelector::start`]). Dropped without [`finish`](Run::finish) or
/// [`fail`](Run::fail) (cancelled, or a fallback), it records nothing.
pub struct Run<'a> {
    sel: &'a PathSelector,
    task: Task,
    path: usize,
    key: Key,
    mpx: f32,
    t0: Instant,
    /// This run's entry in the CPU ledger (`paths/cpu.rs`).
    cpu: Option<u64>,
    done: bool,
    /// Counted in [`PathSelector`]'s `exploring` (snapshot.rs).
    exploring: bool,
}

impl Run<'_> {
    /// Records the run's wall time and its CPU time over all threads (see
    /// [`PathSelector::start`]).
    pub fn finish(mut self) {
        self.done = true;
        let wall = self.t0.elapsed().as_secs_f32() * 1e6;
        let cpu = self.cpu.take().and_then(cpu::close).map(|us| us as f32);
        self.sel
            .record(self.task, self.path, self.key, self.mpx, wall, cpu);
    }

    /// Records a run whose costs the caller measured (e.g. CPU-side time plus a
    /// calibrated GPU share).
    pub fn finish_with(mut self, wall_us: f32, cpu_us: Option<f32>) {
        self.done = true;
        self.sel
            .record(self.task, self.path, self.key, self.mpx, wall_us, cpu_us);
    }

    /// Records only CPU time (see [`PathSelector::record_cpu`]).
    pub fn finish_cpu(mut self, cpu_us: f32) {
        self.done = true;
        self.sel
            .record_cpu(self.task, self.path, self.key, self.mpx, cpu_us);
    }

    /// The path errored: blocklist it (the caller redoes the work on the CPU).
    pub fn fail(mut self, why: &str) {
        self.done = true;
        if self.path > 0 {
            self.sel
                .block(self.task, self.task.engines()[self.path], why);
        }
    }

    pub fn elapsed_us(&self) -> f32 {
        self.t0.elapsed().as_secs_f32() * 1e6
    }
}

impl Drop for Run<'_> {
    fn drop(&mut self) {
        let _ = self.done;
        if let Some(id) = self.cpu.take() {
            cpu::close(id);
        }
        self.sel.inflight.fetch_sub(1, Ordering::Relaxed);
        if self.exploring {
            self.sel.exploring[self.task as usize].fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl std::fmt::Debug for Run<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run")
            .field("task", &self.task)
            .field("path", &self.path)
            .finish()
    }
}

// ---------------------------------------------------------------------------------------------
// Notes: what the last decision on this thread came to (open reports, DESIGN.md §12.7)

/// The last decision of the process's selector on this thread, and whether its owner had to
/// fall back to the CPU reference after all. The loader takes it after each decode so an
/// open's report can say which path ran and why; a `Cell`, so noting costs nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub choice: Choice,
    /// The chosen non-reference path was not used for this request after all.
    pub fallback: Option<Fallback>,
    /// The non-reference path ran and was compared with the CPU reference, which also ran
    /// (its first runs, or a check the selector asked for): the work was done twice.
    pub checked: bool,
    /// The trust of the task's other path (path 1) when the decision was made: a probe that
    /// finishes while the work runs must not change why it ran where it did.
    pub alt_trust: Trust,
    /// A reduced-size decode: its scale and how it was made ([`note_scaled`]).
    pub scaled: Option<Scaled>,
}

/// How a reduced-size level was decoded (JPEG levels scaled in the DCT).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scaled {
    /// The level asked for: 1/`requested` of the full size.
    pub requested: u8,
    /// The reduction the decoder made itself (in the DCT): 1/`dct`. 1 when it decoded at full
    /// size and reduced afterwards ([`ScaleFallback::FullSize`]).
    pub dct: u8,
    pub fallback: Option<ScaleFallback>,
}

/// Why a reduced-size decode did not take its usual way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScaleFallback {
    /// The coding cannot be scaled in the DCT (lossless JPEG): decoded at full size, then
    /// reduced. Costs a full decode.
    FullSize,
    /// The striped decode (row index built first, bounded memory) could not be prepared; the
    /// level was decoded in one pass.
    NoIndex,
}

impl ScaleFallback {
    pub const fn key(self) -> &'static str {
        match self {
            ScaleFallback::FullSize => "full-size",
            ScaleFallback::NoIndex => "no-index",
        }
    }
}

/// Why a chosen non-reference path was not used after all (the CPU reference ran instead).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fallback {
    /// Switched off, or not usable this session (timed out, crashed, failed its check).
    Unavailable,
    /// This input is outside what the engine takes (size, profile, chroma format).
    Unsupported,
    /// The engine was busy with other work.
    Busy,
    /// It failed on this input (and is now blocklisted).
    Failed,
}

impl Fallback {
    pub const fn key(self) -> &'static str {
        match self {
            Fallback::Unavailable => "unavailable",
            Fallback::Unsupported => "unsupported",
            Fallback::Busy => "busy",
            Fallback::Failed => "failed",
        }
    }
}

thread_local! {
    static NOTE: Cell<Option<Note>> = const { Cell::new(None) };
}

/// Takes (and clears) this thread's [`Note`]. Call before work to forget older decisions,
/// and after it to read the one the work made.
pub fn take_note() -> Option<Note> {
    NOTE.with(Cell::take)
}

/// Marks this thread's current [`Note`] as having fallen back to the CPU reference. The first
/// fallback noted for a decision is kept.
pub fn note_fallback(f: Fallback) {
    NOTE.with(|n| {
        if let Some(mut note) = n.get()
            && note.fallback.is_none()
        {
            note.fallback = Some(f);
            n.set(Some(note));
        }
    });
}

/// Notes a reduced-size decode that only the CPU reference can do (the selector is not asked):
/// `task`'s reference path for `pixels` of full-size work, [`Reason::Scaled`]. Replaces this
/// thread's [`Note`].
pub fn note_scaled(task: Task, pixels: u64, scaled: Scaled) {
    let sel = &GLOBAL;
    let req = Request::new(pixels, 0).objective(Objective::Interactive);
    let choice = Choice {
        task,
        path: 0,
        engine: task.engines()[0],
        reason: Reason::Scaled,
        verify: false,
        key: sel.key_of(&req),
        mpx: pixels as f32 / 1e6,
    };
    NOTE.with(|n| {
        n.set(Some(Note {
            choice,
            fallback: None,
            checked: false,
            alt_trust: sel.trust(task, 1),
            scaled: Some(scaled),
        }))
    });
}

/// Marks this thread's current reduced-size [`Note`] as having fallen back (the first one
/// noted is kept).
pub fn note_scale_fallback(f: ScaleFallback) {
    NOTE.with(|n| {
        if let Some(mut note) = n.get()
            && let Some(mut s) = note.scaled
            && s.fallback.is_none()
        {
            s.fallback = Some(f);
            if f == ScaleFallback::FullSize {
                s.dct = 1;
            }
            note.scaled = Some(s);
            n.set(Some(note));
        }
    });
}

/// Marks this thread's current [`Note`] as checked: the CPU reference ran beside the chosen
/// path to compare their output.
pub fn note_checked() {
    NOTE.with(|n| {
        if let Some(mut note) = n.get() {
            note.checked = true;
            n.set(Some(note));
        }
    });
}

/// CPU time and major page faults (pages read from storage) of the calling thread so far
/// (Linux; `None` elsewhere). Two calls around work tell how much of its wall time it spent
/// on the CPU and whether it waited for a memory-mapped file to be read.
pub fn thread_usage() -> Option<ThreadUsage> {
    #[cfg(target_os = "linux")]
    {
        use nix::sys::resource::{UsageWho, getrusage};
        let u = getrusage(UsageWho::RUSAGE_THREAD).ok()?;
        let us = |t: nix::sys::time::TimeVal| t.tv_sec() as f64 * 1e6 + t.tv_usec() as f64;
        Some(ThreadUsage {
            cpu_us: us(u.user_time()) + us(u.system_time()),
            major_faults: u.major_page_faults().max(0) as u64,
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// See [`thread_usage`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThreadUsage {
    pub cpu_us: f64,
    pub major_faults: u64,
}

/// CPU time of the whole process so far, in microseconds (Linux; `None` elsewhere).
pub fn process_cpu_us() -> Option<f32> {
    cpu::process_us().map(|us| us as f32)
}

/// The cache key of a machine: GPU, driver, CPU and app version. A change of any of them
/// redoes the probe and the calibration and forgets the blocklist.
pub fn fingerprint(gpu: &str, driver: &str, cpu: &str, app_version: &str) -> String {
    let clean = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join("_")
            .replace('|', "/")
    };
    format!(
        "{}|{}|{}|{}",
        clean(gpu),
        clean(driver),
        clean(cpu),
        clean(app_version)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(gpu: GpuClass) -> PathSelector {
        let s = PathSelector::new();
        s.set_machine(gpu, 16);
        s
    }

    fn req(o: Objective, mpx: f32, variant: u8) -> Request {
        Request {
            objective: o,
            pixels: (mpx * 1e6) as u64,
            variant,
            can_verify: false,
        }
    }

    /// A selector whose trusted GPU took `gpu_ms` and the CPU `cpu_ms` per 256² CLIP pass.
    fn inference(gpu_ms: f32, cpu_ms: f32) -> (PathSelector, Request) {
        let s = sel(GpuClass::Integrated);
        s.set_trust(Task::Inference, Engine::Gpu, Trust::Trusted);
        let r = Request::new(256 * 256, 0).objective(Objective::Background);
        let key = s.key_of(&r);
        let mpx = r.pixels as f32 / 1e6;
        for _ in 0..8 {
            s.record(
                Task::Inference,
                1,
                key,
                mpx,
                gpu_ms * 1e3,
                Some(gpu_ms * 100.0),
            );
            s.record(
                Task::Inference,
                0,
                key,
                mpx,
                cpu_ms * 1e3,
                Some(cpu_ms * 1e3 * 8.0),
            );
        }
        (s, r)
    }

    fn cpu_runs(s: &PathSelector, r: &Request, n: u32) -> usize {
        (0..n)
            .map(|_| s.choose(Task::Inference, r))
            .filter(|c| c.engine == Engine::Cpu)
            .count()
    }

    #[test]
    fn trusted_clearly_faster_gpu_stops_regular_cpu_inference_runs() {
        // Measured on a Radeon 860M: a CLIP image pass 76 ms on the GPU, 532 ms on the CPU.
        let (s, r) = inference(76.0, 532.0);
        // One re-check this session, then none for the next thousand requests (with 1 in 16
        // that would have been 64 whole CPU passes).
        assert_eq!(cpu_runs(&s, &r, EXPLORE_EVERY * 64), 1);
        let gpu = s.choose(Task::Inference, &r);
        assert_eq!((gpu.engine, gpu.reason), (Engine::Gpu, Reason::Cheaper));
        // A preview says the same and never explores.
        let k = s.background[Task::Inference as usize].load(Ordering::Relaxed);
        for _ in 0..EXPLORE_EVERY * 2 {
            assert_eq!(s.preview(Task::Inference, &r), gpu);
        }
        assert_eq!(
            s.background[Task::Inference as usize].load(Ordering::Relaxed),
            k
        );
        // The re-check comes back once RECHECK_SECS have passed, and only once.
        let key = s.key_of(&r);
        let mut est = [None; MAX_PATHS];
        est[0] = Some(s.estimate(Task::Inference, 0, key, 0.0655));
        est[1] = Some(s.estimate(Task::Inference, 1, key, 0.0655));
        let last = s.rechecked[Task::Inference as usize].load(Ordering::Relaxed) - 1;
        assert!(!s.recheck_due(Task::Inference, 1, 0, &est, last + RECHECK_SECS - 1));
        assert!(s.recheck_due(Task::Inference, 1, 0, &est, last + RECHECK_SECS));
        assert!(!s.recheck_due(Task::Inference, 1, 0, &est, last + RECHECK_SECS + 1));
        // Other tasks still explore 1 in 16 (the request count is per task).
        assert!(s.recheck_due(Task::JpegDecode, 1, 0, &est, last));
        // A new fingerprint (driver update) or "Measure again" starts the session over.
        s.reset_measurements();
        assert_eq!(
            s.rechecked[Task::Inference as usize].load(Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn slow_or_failing_gpu_inference_still_falls_back_to_the_cpu() {
        // Not clearly faster (1.5×): the CPU is explored 1 in 16 as before.
        let (s, r) = inference(300.0, 450.0);
        assert_eq!(s.choose(Task::Inference, &r).engine, Engine::Gpu);
        assert_eq!(cpu_runs(&s, &r, EXPLORE_EVERY * 4 - 1), 4);
        // A GPU that turns slow is timed on its own runs and loses to the CPU.
        let (s, r) = inference(76.0, 532.0);
        let key = s.key_of(&r);
        for _ in 0..32 {
            s.record(Task::Inference, 1, key, 0.0655, 8e6, Some(8e3));
        }
        assert_eq!(cpu_runs(&s, &r, EXPLORE_EVERY), EXPLORE_EVERY as usize);
        // A GPU that fails is blocked: every request runs on the CPU.
        let (s, r) = inference(76.0, 532.0);
        s.block(Task::Inference, Engine::Gpu, "device lost");
        let c = s.choose(Task::Inference, &r);
        assert_eq!((c.engine, c.reason), (Engine::Cpu, Reason::Only));
        assert_eq!(cpu_runs(&s, &r, EXPLORE_EVERY), EXPLORE_EVERY as usize);
    }

    #[test]
    fn cpu_reference_is_the_only_path_until_others_exist() {
        let s = sel(GpuClass::Integrated);
        let c = s.choose(Task::JpegDecode, &req(Objective::Interactive, 24.0, 0));
        assert_eq!((c.engine, c.reason), (Engine::Cpu, Reason::Only));
        assert!(!c.verify);
    }

    #[test]
    fn unverified_paths_are_never_used_interactively() {
        let s = sel(GpuClass::Discrete);
        s.set_available(Task::JpegDecode, Engine::HwDecoder);
        for _ in 0..64 {
            let c = s.choose(
                Task::JpegDecode,
                &req(Objective::Interactive, 24.0, 0).verifying(),
            );
            assert_eq!(c.engine, Engine::Cpu);
        }
        // Background requests that can verify explore it now and then, with `verify` set.
        let explored = (0..EXPLORE_EVERY)
            .map(|_| {
                s.choose(
                    Task::JpegDecode,
                    &req(Objective::Background, 24.0, 0).verifying(),
                )
            })
            .filter(|c| c.engine == Engine::HwDecoder)
            .collect::<Vec<_>>();
        assert_eq!(explored.len(), 1);
        assert!(explored[0].verify);
        assert_eq!(explored[0].reason, Reason::Explore);
        s.verified(Task::JpegDecode, Engine::HwDecoder, true);
        let c = s.choose(Task::JpegDecode, &req(Objective::Interactive, 24.0, 0));
        assert_eq!(
            c.engine,
            Engine::HwDecoder,
            "a dGPU decoder's prior beats the CPU"
        );
    }

    #[test]
    fn blocked_is_sticky_and_falls_back_to_cpu() {
        let s = sel(GpuClass::Discrete);
        s.set_trust(Task::YcbcrToRgb, Engine::Gpu, Trust::Trusted);
        assert_eq!(
            s.choose(Task::YcbcrToRgb, &req(Objective::Interactive, 12.0, 0))
                .engine,
            Engine::Gpu
        );
        let r = s.start(&s.choose(Task::YcbcrToRgb, &req(Objective::Interactive, 12.0, 0)));
        r.fail("device lost");
        assert_eq!(s.trust(Task::YcbcrToRgb, 1), Trust::Blocked);
        s.set_available(Task::YcbcrToRgb, Engine::Gpu);
        s.set_trust(Task::YcbcrToRgb, Engine::Gpu, Trust::Trusted);
        assert_eq!(s.trust(Task::YcbcrToRgb, 1), Trust::Blocked);
        // Even forced on.
        s.set_override(Task::YcbcrToRgb, Override::On);
        assert_eq!(
            s.choose(Task::YcbcrToRgb, &req(Objective::Interactive, 12.0, 0))
                .engine,
            Engine::Cpu
        );
        assert_eq!(s.inflight(), 0);
    }

    #[test]
    fn modes_and_overrides() {
        let s = sel(GpuClass::Integrated);
        s.set_trust(Task::JpegDecode, Engine::HwDecoder, Trust::Trusted);
        let r = req(Objective::Interactive, 24.0, 0);
        // The 860M's decoder is slower than all CPU threads: automatic keeps the CPU.
        assert_eq!(s.choose(Task::JpegDecode, &r).engine, Engine::Cpu);
        s.set_mode(Mode::PreferGpu);
        assert_eq!(s.choose(Task::JpegDecode, &r).reason, Reason::PreferGpu);
        s.set_override(Task::JpegDecode, Override::Off);
        assert_eq!(s.choose(Task::JpegDecode, &r).engine, Engine::Cpu);
        s.set_override(Task::JpegDecode, Override::Auto);
        s.set_mode(Mode::CpuOnly);
        let c = s.choose(Task::JpegDecode, &r);
        assert_eq!((c.engine, c.reason), (Engine::Cpu, Reason::CpuOnly));
        // CPU only still lets a CPU variant compete.
        s.set_trust(Task::PngUnfilter, Engine::CpuParallel, Trust::Trusted);
        assert_eq!(
            s.choose(Task::PngUnfilter, &req(Objective::Interactive, 40.0, 3))
                .engine,
            Engine::CpuParallel
        );
    }

    #[test]
    fn learning_overrides_the_prior() {
        let s = sel(GpuClass::Integrated);
        s.set_trust(Task::JpegDecode, Engine::HwDecoder, Trust::Trusted);
        let r = req(Objective::Interactive, 24.0, 0);
        assert_eq!(s.choose(Task::JpegDecode, &r).engine, Engine::Cpu);
        // A new driver made the decoder fast: real runs teach it.
        let key = s.key_of(&r);
        for _ in 0..8 {
            s.record(Task::JpegDecode, 1, key, 24.0, 8_000.0, Some(2_000.0));
            s.record(Task::JpegDecode, 0, key, 24.0, 30_000.0, Some(300_000.0));
        }
        let c = s.choose(Task::JpegDecode, &r);
        assert_eq!((c.engine, c.reason), (Engine::HwDecoder, Reason::Faster));
        // A neighbouring bucket borrows the measurement.
        let c = s.choose(Task::JpegDecode, &req(Objective::Interactive, 60.0, 0));
        assert_eq!(c.engine, Engine::HwDecoder);
    }

    #[test]
    fn table_round_trip() {
        let s = sel(GpuClass::Discrete);
        s.set_fingerprint("gpu|drv|cpu|1");
        s.set_trust(Task::JpegDecode, Engine::HwDecoder, Trust::Blocked);
        s.set_trust(Task::YcbcrToRgb, Engine::Gpu, Trust::Trusted);
        let key = Key {
            variant: 2,
            bucket: 3,
            busy: true,
        };
        s.record(Task::PngUnfilter, 1, key, 8.0, 20_000.0, Some(90_000.0));
        s.set_calibrated();
        let t = s.to_table();
        let u = PathSelector::new();
        assert!(!u.load_table(&t, Some("other")));
        assert!(u.load_table(&t, Some("gpu|drv|cpu|1")));
        assert_eq!(u.to_table(), t);
        assert!(u.calibrated());
        assert_eq!(u.gpu_class(), GpuClass::Discrete);
        assert_eq!(u.samples(Task::PngUnfilter, 1, key), 1);
        // A new fingerprint forgets the blocklist.
        u.set_fingerprint("gpu|newer driver|cpu|1");
        assert_eq!(u.trust(Task::JpegDecode, 1), Trust::Absent);
        assert!(!u.calibrated());
    }

    #[test]
    fn buckets_and_variants() {
        assert_eq!(bucket(0), 0);
        assert_eq!(bucket(1 << 20), 2);
        assert_eq!(bucket(24_000_000), 4);
        assert_eq!(bucket(u64::MAX), 5);
        assert_eq!(png_variant(100, 50), 0);
        assert_eq!(png_variant(40 << 20, 1 << 20), 3);
        assert_eq!(png_variant(1, 0), 0);
    }

    #[test]
    fn objective_follows_the_thread() {
        assert_eq!(current_objective(), Objective::Interactive);
        set_current_objective(Objective::Background);
        assert_eq!(Request::new(10, 0).objective, Objective::Background);
        set_current_objective(Objective::Interactive);
    }

    #[test]
    fn notes_keep_the_decision_and_reduced_size_decodes() {
        // A local selector leaves no note; the process's one does, with the other path's
        // trust as it was when deciding.
        let _ = take_note();
        let _ = sel(GpuClass::Integrated)
            .choose(Task::HeicDecode, &req(Objective::Interactive, 12.0, 0));
        assert_eq!(take_note(), None);
        let c = selector().choose(Task::HeicDecode, &req(Objective::Interactive, 12.0, 0));
        let n = take_note().unwrap();
        assert_eq!(n.choice, c);
        assert_eq!(n.alt_trust, selector().trust(Task::HeicDecode, 1));
        assert_eq!(n.scaled, None);
        // A scaled level: the reference, with its scale; a full-size fallback is noted once.
        note_scaled(
            Task::JpegDecode,
            24_000_000,
            Scaled {
                requested: 4,
                dct: 4,
                fallback: None,
            },
        );
        note_scale_fallback(ScaleFallback::FullSize);
        note_scale_fallback(ScaleFallback::NoIndex);
        let n = take_note().unwrap();
        assert_eq!(
            (n.choice.engine, n.choice.reason, n.choice.path),
            (Engine::Cpu, Reason::Scaled, 0)
        );
        assert_eq!(
            n.scaled,
            Some(Scaled {
                requested: 4,
                dct: 1,
                fallback: Some(ScaleFallback::FullSize)
            })
        );
        // Without a scaled note there is nothing to mark.
        note_scale_fallback(ScaleFallback::NoIndex);
        assert_eq!(take_note(), None);
    }

    fn cell_rate(s: &PathSelector, task: Task, path: usize, key: Key) -> f32 {
        f32::from_bits(
            s.cells[cell_index(task, path, key)]
                .wall
                .load(Ordering::Relaxed),
        )
    }

    /// A run faster than the prior's setup alone (ViT-Ti on an iGPU: 8 ms against a 20 ms
    /// setup) used to store a wall rate of 0, and the estimate stayed at the setup. Now the
    /// cell is positive and the estimate at the measured size is the measurement, whether the
    /// run was faster or slower than the prior.
    #[test]
    fn fast_runs_never_store_a_zero_cell() {
        let s = sel(GpuClass::Integrated);
        let px = 224 * 224;
        let mpx = px as f32 / 1e6;
        let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
        let c = s.calibration(Task::Inference, gpu, px, 0);
        let pr = s.prior_of(Task::Inference, gpu, c.key);
        assert!(
            pr.setup_us > 8_000.0,
            "the case needs a setup above the run"
        );
        for wall in [8_000.0, 1.0, 0.0] {
            let s = sel(GpuClass::Integrated);
            s.record(Task::Inference, gpu, c.key, mpx, wall, None);
            let rate = cell_rate(&s, Task::Inference, gpu, c.key);
            assert!(rate > 0.0, "{wall} µs stored {rate}");
            let e = s.estimate(Task::Inference, gpu, c.key, mpx);
            assert_eq!(e.source, Source::Measured);
            assert!(
                (e.wall_us - wall).abs() <= 0.01 * wall + 1.0,
                "{wall} µs estimated as {}",
                e.wall_us
            );
        }
        // Between the setup and the prior's total, and slower than the prior: exact too.
        let total = pr.setup_us + pr.wall_per_mpx * mpx;
        for wall in [pr.setup_us + 1.0, total * 0.9, total, total * 3.0] {
            let s = sel(GpuClass::Integrated);
            s.record(Task::Inference, gpu, c.key, mpx, wall, None);
            assert!(cell_rate(&s, Task::Inference, gpu, c.key) > 0.0);
            let e = s.estimate(Task::Inference, gpu, c.key, mpx);
            assert!(
                (e.wall_us - wall).abs() <= 1e-3 * wall,
                "{wall} → {}",
                e.wall_us
            );
        }
        // A slow run keeps the prior's whole setup: the old split, unchanged.
        let s2 = sel(GpuClass::Integrated);
        s2.record(Task::Inference, gpu, c.key, mpx, total * 3.0, None);
        let rate = cell_rate(&s2, Task::Inference, gpu, c.key);
        assert!((rate - (total * 3.0 - pr.setup_us) / mpx).abs() <= 1e-3 * rate);
        // Averaged with faster runs the cell stays positive.
        let s = sel(GpuClass::Integrated);
        for wall in [0.0, 5_000.0, 0.0, 30_000.0, 0.0, 0.0, 0.0] {
            s.record(Task::Inference, gpu, c.key, mpx, wall, None);
            assert!(cell_rate(&s, Task::Inference, gpu, c.key) > 0.0);
            let e = s.estimate(Task::Inference, gpu, c.key, 4.0 * mpx);
            assert!(e.wall_us.is_finite() && e.wall_us > 0.0);
        }
        // A zero cell in an old cache file is not a measurement.
        let s = sel(GpuClass::Integrated);
        assert!(s.load_table(
            "# Specular path timings (DESIGN.md §12.3.1), version 1\n\
             machine gpu=integrated threads=16\n\
             cell inference gpu 0 0 idle 2 0.0 0 0.0\n\
             cell inference cpu 0 0 idle 2 -5 0 0.0\n",
            None
        ));
        assert_eq!(s.samples(Task::Inference, gpu, c.key), 0);
        assert_eq!(s.samples(Task::Inference, 0, c.key), 0);
        assert_eq!(
            s.estimate(Task::Inference, gpu, c.key, mpx).source,
            Source::Prior
        );
    }

    #[test]
    fn report_does_not_explore() {
        let s = sel(GpuClass::Integrated);
        s.set_trust(Task::PngUnfilter, Engine::CpuParallel, Trust::Trusted);
        for _ in 0..40 {
            let lines = s.report_lines(24.0);
            assert!(
                lines.iter().any(|l| l.starts_with("png_unfilter")),
                "{lines:?}"
            );
        }
        assert_eq!(
            s.background[Task::PngUnfilter as usize].load(Ordering::Relaxed),
            0
        );
    }
}
