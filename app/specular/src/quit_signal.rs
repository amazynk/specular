//! Ctrl+C in a terminal and `kill` (SIGINT, SIGTERM, SIGHUP on Unix; the console Ctrl+C,
//! Ctrl+Break and close events on Windows) end the viewer like closing its window. The handler
//! only wakes the event loop, which then calls `exit()`, so `exiting` still shuts down the
//! drag-source thread, the accessibility adapter and the surface while the display connection
//! is open (app.rs).
//!
//! A second signal before the loop has exited ends the process at once, in case the loop is
//! stuck.

use std::sync::atomic::{AtomicBool, Ordering};

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// Exit status after a second signal: 128 + SIGINT, as a shell reports Ctrl+C on Unix;
/// STATUS_CONTROL_C_EXIT on Windows.
const FORCED_EXIT: i32 = if cfg!(windows) {
    0xC000_013A_u32.cast_signed()
} else {
    130
};

/// Whether a quit signal has arrived.
pub fn requested() -> bool {
    REQUESTED.load(Ordering::Acquire)
}

/// Starts listening; `wake` wakes the event loop, which checks [`requested`]. ctrlc runs the
/// closure on its own thread, never inside the signal or console-control handler.
pub fn install(wake: impl Fn() + Send + 'static) {
    let set = ctrlc::set_handler(move || {
        if REQUESTED.swap(true, Ordering::AcqRel) {
            std::process::exit(FORCED_EXIT);
        }
        wake();
    });
    if let Err(e) = set {
        specular_log::warn!(target: specular_log::STARTUP, "cannot handle Ctrl+C: {e}");
    }
}
