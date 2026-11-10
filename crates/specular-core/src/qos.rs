//! The hook between [`JobPool`](crate::JobPool) priorities and the OS scheduler.
//!
//! Each worker tells the OS how urgent the job it is about to run is. On macOS that sets the
//! thread's QoS class, which on Apple Silicon is what places work on performance or efficiency
//! cores; on Linux hybrid CPUs, [`placement`](crate::placement) pins the worker to
//! the fast or slow cores. The call is free when the class does not change.

use specular_platform::ThreadClass;

use crate::Priority;
use crate::jobs::FOREGROUND;
use crate::placement::CoreSet;

/// The scheduling class for a job of `p`. A background pool's jobs (thumbnails) stay below
/// the viewer's: its visible ones are `Utility`, the rest `Background`.
pub(crate) fn class_for(p: Priority, background: bool) -> ThreadClass {
    match (p, background) {
        (Priority::Immediate | Priority::Visible | Priority::Near, false) => {
            ThreadClass::UserInitiated
        }
        (Priority::Prefetch, false) => ThreadClass::Utility,
        (p, true) if p >= FOREGROUND => ThreadClass::Utility,
        _ => ThreadClass::Background,
    }
}

/// Called by a worker of a background pool once, before its first job.
pub(crate) fn background_worker_starting() {
    specular_platform::lower_thread_priority();
}

/// Called by a worker right before it runs a job.
pub(crate) fn job_starting(p: Priority, background: bool) {
    specular_platform::set_thread_class(class_for(p, background));
    // Path choices on this thread follow the job (DESIGN.md §12.3.1).
    crate::paths::set_current_objective(if p >= FOREGROUND {
        crate::paths::Objective::Interactive
    } else {
        crate::paths::Objective::Background
    });
    crate::placement::place_current_thread(if p >= FOREGROUND {
        CoreSet::Performance
    } else {
        CoreSet::Efficiency
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreground_jobs_get_the_fast_cores() {
        assert_eq!(
            class_for(Priority::Immediate, false),
            ThreadClass::UserInitiated
        );
        assert_eq!(class_for(Priority::Near, false), ThreadClass::UserInitiated);
        assert_eq!(class_for(Priority::Prefetch, false), ThreadClass::Utility);
        assert_eq!(class_for(Priority::Idle, false), ThreadClass::Background);
    }

    #[test]
    fn background_pools_stay_below_the_viewer() {
        assert_eq!(class_for(Priority::Visible, true), ThreadClass::Utility);
        assert_eq!(class_for(Priority::Prefetch, true), ThreadClass::Background);
        assert_eq!(class_for(Priority::Idle, true), ThreadClass::Background);
    }
}
