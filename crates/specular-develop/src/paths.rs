//! Develop's entry in the `PathSelector` (DESIGN.md §12.3.1, `Task::Develop`): renders run on
//! the hardware GPU, or on the software adapter (lavapipe, WARP), the reference path that is
//! always there. The selector picks between them from timings; the
//! performance gate (`benches/develop`) uses it to pick its adapter and teaches it its
//! export timings.

use specular_core::paths::{Engine, GpuClass, PathSelector, Request, Task, Trust};

/// Which adapter Develop renders on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adapter {
    /// The machine's GPU.
    Hardware,
    /// The software adapter (lavapipe, WARP).
    Software,
}

impl Adapter {
    /// The selector's engine for this adapter.
    pub fn engine(self) -> Engine {
        match self {
            Adapter::Hardware => Engine::Gpu,
            Adapter::Software => Engine::Cpu,
        }
    }

    fn path(self) -> usize {
        Task::Develop.path_of(self.engine()).unwrap_or(0)
    }
}

/// Registers the hardware path: trusted when a hardware device opened (every op is checked
/// against its CPU reference by the tests, and the software adapter runs the same shaders),
/// blocked when there is none. The software adapter needs no registering.
pub fn register(sel: &PathSelector, hardware: Option<GpuClass>) {
    match hardware {
        Some(class) => {
            if sel.gpu_class() == GpuClass::Unknown {
                sel.set_machine(
                    class,
                    std::thread::available_parallelism().map_or(1, usize::from),
                );
            }
            sel.set_trust(Task::Develop, Engine::Gpu, Trust::Trusted);
        }
        None => sel.block(Task::Develop, Engine::Gpu, "no hardware GPU"),
    }
}

/// The adapter for rendering `pixels` (a source of that size).
pub fn choose(sel: &PathSelector, pixels: u64) -> Adapter {
    let c = sel.choose(Task::Develop, &Request::new(pixels, 0));
    if c.engine == Engine::Gpu {
        Adapter::Hardware
    } else {
        Adapter::Software
    }
}

/// Records one render of `pixels` on `adapter` that took `wall_ms`.
pub fn record(sel: &PathSelector, adapter: Adapter, pixels: u64, wall_ms: f64) {
    let c = sel.calibration(Task::Develop, adapter.path(), pixels, 0);
    sel.record(
        Task::Develop,
        adapter.path(),
        c.key,
        pixels as f32 / 1e6,
        (wall_ms * 1e3) as f32,
        None,
    );
}

/// The selector's predicted wall milliseconds for `pixels` on `adapter`.
pub fn estimate_ms(sel: &PathSelector, adapter: Adapter, pixels: u64) -> f64 {
    let c = sel.calibration(Task::Develop, adapter.path(), pixels, 0);
    f64::from(
        sel.estimate(Task::Develop, adapter.path(), c.key, pixels as f32 / 1e6)
            .wall_us,
    ) / 1e3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gpu_is_chosen_and_learns_its_timings() {
        let sel = PathSelector::new();
        register(&sel, Some(GpuClass::Integrated));
        assert_eq!(choose(&sel, 24_000_000), Adapter::Hardware);
        // An export of 24 MP measured at 2 s on the GPU: the estimate follows it.
        for _ in 0..8 {
            record(&sel, Adapter::Hardware, 24_000_000, 2000.0);
        }
        let e = estimate_ms(&sel, Adapter::Hardware, 24_000_000);
        assert!((e - 2000.0).abs() < 200.0, "{e}");
    }

    #[test]
    fn without_a_gpu_the_software_adapter_renders() {
        let sel = PathSelector::new();
        register(&sel, None);
        assert_eq!(choose(&sel, 24_000_000), Adapter::Software);
        // A GPU much slower than the software adapter (a broken driver) loses to it.
        let sel = PathSelector::new();
        register(&sel, Some(GpuClass::Integrated));
        for _ in 0..8 {
            record(&sel, Adapter::Hardware, 24_000_000, 600_000.0);
            record(&sel, Adapter::Software, 24_000_000, 20_000.0);
        }
        assert_eq!(choose(&sel, 24_000_000), Adapter::Software);
    }
}
