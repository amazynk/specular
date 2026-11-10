//! What the hardware report says, gathered once: `--hw-report` prints it, Settings → Advanced
//! → Hardware report shows it ([`super::panel`]).

use specular_core::HardwareProfile;
use specular_core::paths::{Mode, PathSelector, TaskReport, Trust};
use specular_gfx::Gpu;
use specular_gfx::wgpu::{DeviceType, Features};
use specular_hwdec::Capabilities;
use specular_platform::{CoreCluster, PowerState, ScreenInfo};

/// The megapixel size the path selector's estimates are shown for.
pub(crate) const REPORT_MPX: f32 = 24.0;

/// The graphics adapter.
pub(crate) struct GpuFacts {
    pub(crate) name: String,
    pub(crate) backend: String,
    pub(crate) driver: String,
    pub(crate) kind: DeviceType,
    pub(crate) init_ms: f64,
    pub(crate) max_texture: u32,
    pub(crate) max_layers: u32,
    /// Block compression formats: name, supported.
    pub(crate) compression: Vec<(&'static str, bool)>,
    pub(crate) unified: bool,
    /// What the platform says about unified memory, if it says.
    pub(crate) platform_unified: Option<bool>,
}

impl GpuFacts {
    fn of(gpu: &Gpu) -> GpuFacts {
        let f = gpu.adapter.features();
        GpuFacts {
            name: gpu.info.name.clone(),
            backend: format!("{:?}", gpu.info.backend),
            driver: format!("{} {}", gpu.info.driver, gpu.info.driver_info)
                .trim()
                .to_owned(),
            kind: gpu.info.device_type,
            init_ms: gpu.init_ms,
            max_texture: gpu.max_texture_size(),
            max_layers: gpu.limits.max_texture_array_layers,
            compression: [
                (Features::TEXTURE_COMPRESSION_BC, "BC"),
                (Features::TEXTURE_COMPRESSION_ASTC, "ASTC"),
                (Features::TEXTURE_COMPRESSION_ETC2, "ETC2"),
            ]
            .into_iter()
            .map(|(b, n)| (n, f.contains(b)))
            .collect(),
            unified: gpu.unified_memory(),
            platform_unified: specular_platform::unified_memory(),
        }
    }
}

/// The path selector's state and its choices at [`REPORT_MPX`].
pub(crate) struct PathFacts {
    pub(crate) mode: Mode,
    pub(crate) gpu_class: &'static str,
    pub(crate) threads: u16,
    pub(crate) calibrated: bool,
    pub(crate) power_saving: bool,
    /// Only the tasks with a path besides the CPU reference on this machine.
    pub(crate) tasks: Vec<TaskReport>,
}

/// Everything in the report except the performance insights (which have their own text).
pub(crate) struct Facts {
    pub(crate) hw: HardwareProfile,
    pub(crate) clusters: Vec<CoreCluster>,
    pub(crate) power: PowerState,
    pub(crate) gpu: Result<GpuFacts, String>,
    pub(crate) screen: Option<ScreenInfo>,
    /// The hardware decoders: `None` while still being probed, `Some(None)` when there are
    /// none.
    pub(crate) hwdec: Option<Option<&'static Capabilities>>,
    pub(crate) paths: PathFacts,
    /// The CPU instruction sets this build's fast paths use.
    pub(crate) cpu_fast_paths: Vec<&'static str>,
}

impl Facts {
    /// Reads the machine. The selector must know it already
    /// ([`crate::app::performance::describe_machine`]).
    pub(crate) fn gather(
        hw: HardwareProfile,
        gpu: Result<&Gpu, String>,
        hwdec: Option<Option<&'static Capabilities>>,
    ) -> Facts {
        let sel = specular_core::paths::selector();
        Facts {
            hw,
            clusters: specular_platform::core_clusters(),
            power: specular_platform::power_state(),
            gpu: gpu.map(GpuFacts::of),
            screen: specular_platform::main_screen(),
            hwdec,
            paths: PathFacts {
                mode: sel.mode(),
                gpu_class: sel.gpu_class().key(),
                threads: sel.threads(),
                calibrated: sel.calibrated(),
                power_saving: sel.power_saving(),
                tasks: report(sel)
                    .into_iter()
                    .filter(|r| r.paths.iter().skip(1).any(|p| p.trust != Trust::Absent))
                    .collect(),
            },
            cpu_fast_paths: cpu_fast_paths(),
        }
    }
}

/// The selector's report at [`REPORT_MPX`] (tasks with a unit of their own, such as
/// inference on a thumbnail, are estimated at that unit's size).
fn report(sel: &PathSelector) -> Vec<TaskReport> {
    sel.report(REPORT_MPX)
}

/// The CPU instruction sets this build's fast paths use (compiled in, so always present).
fn cpu_fast_paths() -> Vec<&'static str> {
    let mut v = Vec::new();
    if cfg!(target_feature = "sse4.2") {
        v.push("SSE4.2");
    }
    if cfg!(target_feature = "avx2") {
        v.push("AVX2");
    }
    if cfg!(target_feature = "fma") {
        v.push("FMA");
    }
    if cfg!(target_feature = "bmi2") {
        v.push("BMI2");
    }
    if cfg!(target_feature = "avx512f") {
        v.push("AVX-512");
    }
    if cfg!(target_feature = "neon") {
        v.push("NEON");
    }
    v
}
