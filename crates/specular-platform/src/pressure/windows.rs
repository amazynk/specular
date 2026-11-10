//! Memory pressure on Windows from the memory resource notifications
//! (`CreateMemoryResourceNotification`): the "low" object is signalled while available physical
//! memory is below the system's low threshold (about 32 MB per 4 GB, at most 64 MB), the "high"
//! object while it is above the high threshold (about three times that). Low means
//! [`MemoryPressure::Critical`], neither means [`MemoryPressure::Warn`], high means normal.
//!
//! Both are states, not edges: nothing signals when memory stops being plentiful or stops being
//! low. The thread waits on "low" with a timeout, [`CALM_PERIOD`] while normal and
//! [`RAISED_PERIOD`] on a warning, and only sleeps while critical (the object stays signalled);
//! after each wake it queries both and feeds [`Hysteresis`].

use std::time::Duration;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Memory::{
    CreateMemoryResourceNotification, HighMemoryResourceNotification,
    LowMemoryResourceNotification, MEMORY_RESOURCE_NOTIFICATION_TYPE,
    QueryMemoryResourceNotification,
};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::BOOL;

use super::Hysteresis;
use crate::{MemoryPressure, report_memory_pressure};

/// How often "high" is queried while the level is normal.
const CALM_PERIOD: Duration = Duration::from_secs(5);
/// How often both are queried while the level is raised.
const RAISED_PERIOD: Duration = Duration::from_secs(2);

pub(crate) fn start() {
    let spawned = specular_thread::builder("plat-memory").spawn(|| super::supervised(run));
    if let Err(e) = spawned {
        specular_log::debug!(target: specular_log::CACHE, "memory pressure: no watcher thread: {e}");
    }
}

/// A notification object, kept open for the life of the process (never closed).
fn notification(kind: MEMORY_RESOURCE_NOTIFICATION_TYPE) -> Option<HANDLE> {
    // SAFETY: `kind` is one of the two documented notification types; the call has no other
    // inputs and returns a new handle or an error.
    #[allow(unsafe_code)]
    unsafe { CreateMemoryResourceNotification(kind) }.ok()
}

/// Whether the notification object is signalled right now.
fn signalled(h: HANDLE) -> Option<bool> {
    let mut state = BOOL::default();
    // SAFETY: `h` is a memory resource notification handle that is never closed, and `state`
    // is a live, writable BOOL the call fills in.
    #[allow(unsafe_code)]
    unsafe { QueryMemoryResourceNotification(h, &mut state) }.ok()?;
    Some(state.as_bool())
}

/// The level the two states show.
fn level_of(low: bool, high: bool) -> MemoryPressure {
    if low {
        MemoryPressure::Critical
    } else if high {
        MemoryPressure::Normal
    } else {
        MemoryPressure::Warn
    }
}

fn run() {
    let (Some(low), Some(high)) = (
        notification(LowMemoryResourceNotification),
        notification(HighMemoryResourceNotification),
    ) else {
        specular_log::debug!(target: specular_log::CACHE, "memory pressure: no resource notifications");
        return;
    };
    specular_log::debug!(target: specular_log::CACHE, "memory-pressure watcher started");
    let mut level = Hysteresis::default();
    loop {
        specular_log::service::trip();
        match level.level() {
            // "low" stays signalled while memory is low: waiting on it would return at once.
            MemoryPressure::Critical => std::thread::sleep(RAISED_PERIOD),
            l => {
                let period = if l == MemoryPressure::Normal {
                    CALM_PERIOD
                } else {
                    RAISED_PERIOD
                };
                // SAFETY: `low` is a notification handle that is never closed; the wait only
                // blocks this thread, for at most `period`.
                #[allow(unsafe_code)]
                let _ = unsafe { WaitForSingleObject(low, period.as_millis() as u32) };
            }
        }
        let (Some(l), Some(h)) = (signalled(low), signalled(high)) else {
            continue;
        };
        let before = level.level();
        let now = level.step(level_of(l, h));
        if now != before {
            report_memory_pressure(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_to_levels() {
        assert_eq!(level_of(true, false), MemoryPressure::Critical);
        assert_eq!(level_of(false, false), MemoryPressure::Warn);
        assert_eq!(level_of(false, true), MemoryPressure::Normal);
        // Live objects exist on every supported Windows.
        let low = notification(LowMemoryResourceNotification).unwrap();
        assert!(signalled(low).is_some());
    }
}
