//! P/E-core placement on hybrid CPUs (Linux; elsewhere the OS decides).
//!
//! [`init`] takes [`HardwareProfile::performance_cpus`](crate::HardwareProfile). After that,
//! each [`JobPool`](crate::JobPool) worker pins itself before a job: foreground jobs to the
//! fast cores and background jobs to the slow ones, so a prefetch never takes a fast core
//! from the image the user is looking at. Only the worker thread moves. Decoders' rayon
//! helpers keep every core, because their parallel stages want throughput more than a
//! fast core: [`init`] builds rayon's global pool from the (unpinned) calling thread, since
//! a thread inherits the affinity of the thread that spawns it and the pool would otherwise
//! be spawned by the first, pinned, decode worker. Threads a decoder spawns itself (scoped
//! helpers) do inherit the job's class. A worker repins only when its class changes, so this
//! costs one syscall per change.
//!
//! On macOS, [`qos`](crate::qos) does the same through QoS classes, and this module is a
//! no-op there.

use std::sync::OnceLock;

/// Where a job's thread may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreSet {
    /// The fastest core class.
    Performance,
    /// The other cores (efficiency cores, Zen 5c).
    Efficiency,
}

#[derive(Debug)]
struct Placement {
    fast: Vec<usize>,
    slow: Vec<usize>,
}

static PLACEMENT: OnceLock<Option<Placement>> = OnceLock::new();

/// Turns placement on with `performance_cpus`, the fast CPUs of a hybrid CPU. Does nothing
/// when that list is empty (a uniform CPU), when `SPECULAR_CORE_PLACEMENT=off`, or when the
/// process may not run on both classes (`taskset`, cgroups). Only the first call counts.
/// Returns whether placement is active.
pub fn init(performance_cpus: &[usize]) -> bool {
    PLACEMENT
        .get_or_init(|| {
            if performance_cpus.is_empty()
                || std::env::var_os("SPECULAR_CORE_PLACEMENT").is_some_and(|v| v == "off")
            {
                return None;
            }
            let allowed = sys::allowed()?;
            let fast: Vec<usize> = allowed
                .iter()
                .copied()
                .filter(|c| performance_cpus.contains(c))
                .collect();
            let slow: Vec<usize> = allowed
                .iter()
                .copied()
                .filter(|c| !performance_cpus.contains(c))
                .collect();
            if fast.is_empty() || slow.is_empty() {
                return None;
            }
            specular_log::debug!(target: specular_log::JOBS, "core placement: fast {fast:?}, slow {slow:?}");
            sys::unpinned_rayon_pool();
            Some(Placement { fast, slow })
        })
        .is_some()
}

/// Moves the calling thread onto `set` if placement is active and the thread is elsewhere.
pub fn place_current_thread(set: CoreSet) {
    thread_local! {
        static CURRENT: std::cell::Cell<Option<CoreSet>> = const { std::cell::Cell::new(None) };
    }
    let Some(Some(p)) = PLACEMENT.get() else {
        return;
    };
    if CURRENT.get() == Some(set) {
        return;
    }
    let cpus = match set {
        CoreSet::Performance => &p.fast,
        CoreSet::Efficiency => &p.slow,
    };
    if sys::pin(cpus) {
        CURRENT.set(Some(set));
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use nix::sched::{CpuSet, sched_getaffinity, sched_setaffinity};
    use nix::unistd::Pid;

    /// The CPUs this process may use.
    pub fn allowed() -> Option<Vec<usize>> {
        let set = sched_getaffinity(Pid::from_raw(0)).ok()?;
        Some(
            (0..CpuSet::count())
                .filter(|&c| set.is_set(c).unwrap_or(false))
                .collect(),
        )
    }

    /// Restricts the calling thread to `cpus` (pid 0 is the calling thread).
    pub fn pin(cpus: &[usize]) -> bool {
        let mut set = CpuSet::new();
        for &c in cpus {
            if set.set(c).is_err() {
                return false;
            }
        }
        sched_setaffinity(Pid::from_raw(0), &set).is_ok()
    }

    /// Builds rayon's global pool now, from this unpinned thread, so its threads may run on
    /// every allowed CPU. Without this, every parallel decode was confined to the fast CPUs
    /// (measured: 14 rayon threads on 8 CPUs of 16 on a Ryzen AI 7 PRO 350). Already built
    /// (another caller was first) is fine; the pool's threads are named like the macOS pool.
    pub fn unpinned_rayon_pool() {
        let _ = specular_thread::rayon_pool(specular_thread::GLOBAL_POOL)
            .panic_handler(crate::isolate::rayon_panic_handler)
            .build_global();
    }
}

#[cfg(not(target_os = "linux"))]
mod sys {
    pub fn allowed() -> Option<Vec<usize>> {
        None
    }
    pub fn pin(_: &[usize]) -> bool {
        false
    }
    pub fn unpinned_rayon_pool() {}
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    #[cfg(target_os = "linux")]
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn pinning_round_trips() {
        let all = sys::allowed().expect("affinity");
        assert!(!all.is_empty());
        let first = all[0];
        std::thread::spawn(move || {
            assert!(sys::pin(&[first]));
            assert_eq!(sys::allowed().unwrap(), vec![first]);
        })
        .join()
        .unwrap();
    }
}
