//! Memory pressure on Linux (PSI, cgroup v2 events) and Windows (memory resource
//! notifications); macOS: `macos/pressure.rs`. Each source runs on its own thread, blocked in
//! the OS until something changes (or sampling every couple of seconds while the level is
//! raised, or where the OS gives no events), and reports through `report_memory_pressure`,
//! which wakes the UI thread.

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(windows)]
pub(crate) mod windows;

use crate::MemoryPressure;

/// Starts the platform's source on a helper thread (once; `watch_memory_pressure` makes sure).
pub(crate) fn watch() {
    #[cfg(target_os = "linux")]
    linux::start();
    #[cfg(windows)]
    windows::start();
}

/// Runs a source's loop (`plat-memory`) behind a boundary: a panic drops the level back to
/// normal, so a stale warning does not keep caches trimmed, and the source restarts after a
/// back-off.
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn supervised(run: fn()) {
    specular_log::service::supervise(
        "plat-memory",
        specular_log::service::Policy::RESTART,
        || false,
        run,
        |_| crate::report_memory_pressure(MemoryPressure::Normal),
    );
}

/// Calm samples in a row before the level steps down one notch.
pub(crate) const CALM_SAMPLES: u32 = 5;

/// The level a source reports: up at once to whatever a sample shows, down one notch only
/// after [`CALM_SAMPLES`] samples in a row below the current level, so a burst of reclaim does
/// not toggle caches on and off.
#[derive(Debug, Default)]
pub(crate) struct Hysteresis {
    level: MemoryPressure,
    calm: u32,
}

impl Hysteresis {
    pub(crate) fn level(&self) -> MemoryPressure {
        self.level
    }

    /// Feeds one sample's level; returns the reported level.
    pub(crate) fn step(&mut self, sample: MemoryPressure) -> MemoryPressure {
        if sample >= self.level {
            self.level = sample;
            self.calm = 0;
        } else {
            self.calm += 1;
            if self.calm >= CALM_SAMPLES {
                self.level = match self.level {
                    MemoryPressure::Critical => MemoryPressure::Warn,
                    _ => MemoryPressure::Normal,
                };
                self.calm = 0;
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MemoryPressure::*;

    #[test]
    fn up_at_once_down_slowly() {
        let mut h = Hysteresis::default();
        assert_eq!(h.step(Normal), Normal);
        assert_eq!(h.step(Critical), Critical);
        for _ in 1..CALM_SAMPLES {
            assert_eq!(h.step(Normal), Critical);
        }
        // A raised sample in between starts the count again.
        assert_eq!(h.step(Critical), Critical);
        for _ in 1..CALM_SAMPLES {
            assert_eq!(h.step(Warn), Critical);
        }
        assert_eq!(h.step(Warn), Warn);
        for _ in 1..CALM_SAMPLES {
            assert_eq!(h.step(Normal), Warn);
        }
        assert_eq!(h.step(Normal), Normal);
        assert_eq!(h.step(Warn), Warn);
        assert_eq!(h.level(), Warn);
    }
}
