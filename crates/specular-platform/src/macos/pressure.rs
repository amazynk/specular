//! Memory-pressure notifications from a libdispatch source (`DISPATCH_SOURCE_TYPE_MEMORYPRESSURE`).
//! The handler runs on a global dispatch queue; it records the level and wakes the UI thread.

use std::sync::OnceLock;

use dispatch2::{
    DispatchObject, DispatchRetained, DispatchSource, dispatch_source_memorypressure_flags_t,
};

use crate::{MemoryPressure, set_memory_pressure};

type Wake = Box<dyn Fn() + Send + Sync>;

/// The source (kept alive for the life of the process) and the wake-up callback.
static WATCH: OnceLock<(DispatchRetained<DispatchSource>, Wake)> = OnceLock::new();

pub(crate) fn watch(wake: Wake) {
    let mut fresh = false;
    WATCH.get_or_init(|| {
        fresh = true;
        let mask = dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_NORMAL.0
            | dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_WARN.0
            | dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_CRITICAL.0;
        let source = create(mask as usize);
        source.set_event_handler_f(on_event);
        (source, wake)
    });
    if fresh && let Some((source, _)) = WATCH.get() {
        // Sources start suspended; the handler can find WATCH from now on.
        source.resume();
        specular_log::debug!(target: specular_log::CACHE, "memory-pressure watcher started");
    }
}

#[allow(unsafe_code)]
fn create(mask: usize) -> DispatchRetained<DispatchSource> {
    let kind = (&raw const dispatch2::_dispatch_source_type_memorypressure).cast_mut();
    // SAFETY: `kind` is the address of libdispatch's memory-pressure source type; for that type
    // the handle must be 0 and the mask a combination of DISPATCH_MEMORYPRESSURE_* flags, which
    // it is. `None` targets the default-priority global queue. dispatch_source_create returns
    // NULL only for invalid arguments.
    unsafe { DispatchSource::new(kind, 0, mask, None) }
}

/// The source's event handler (context unused: the source lives in `WATCH`).
extern "C" fn on_event(_context: *mut std::ffi::c_void) {
    let Some((source, wake)) = WATCH.get() else {
        return;
    };
    let flags = source.data();
    let level = if flags
        & dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_CRITICAL.0 as usize
        != 0
    {
        MemoryPressure::Critical
    } else if flags
        & dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_WARN.0 as usize
        != 0
    {
        MemoryPressure::Warn
    } else {
        MemoryPressure::Normal
    };
    if set_memory_pressure(level) {
        specular_log::info!(target: specular_log::CACHE, "memory pressure: {level:?}");
        wake();
    }
}
