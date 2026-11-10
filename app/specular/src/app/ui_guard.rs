//! The UI thread's panic boundary: a panic while handling one event loop callback costs that
//! callback, not the process.
//!
//! winit calls the app from inside the platform's event loop (on macOS an AppKit callback that
//! cannot unwind), so a panic that got out of [`ApplicationHandler`](winit::application::ApplicationHandler)
//! aborted the process ("panic in a function that cannot unwind"). Each callback now runs
//! inside [`specular_core::isolate::run_reported`]: a panic is logged with its location and
//! gets a crash report with its backtrace (in the crash folder, not offered at the next
//! start), the rest of that callback is skipped, the user sees a warning, and the next event
//! goes on as usual.
//!
//! A UI that panics on every frame would only spin and warn, so after [`GIVE_UP_AFTER`] panics
//! within [`GIVE_UP_WINDOW`] the boundary panics again outside it: the old path, which writes a
//! crash report and ends the app (and the next start offers the report). So does a panic before
//! the window exists, which would otherwise leave a process without one.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use specular_i18n::t;

use super::App;

/// Panics within [`GIVE_UP_WINDOW`] after which the UI is taken to be broken for good.
const GIVE_UP_AFTER: usize = 5;
const GIVE_UP_WINDOW: Duration = Duration::from_secs(30);

thread_local! {
    /// When the recent caught UI panics happened (the UI thread's only).
    static RECENT: RefCell<VecDeque<Instant>> = const { RefCell::new(VecDeque::new()) };
}

impl App {
    /// Runs the event loop callback `f` (`what` names it in the log) inside the UI boundary.
    pub(super) fn ui_guard(&mut self, what: &str, f: impl FnOnce(&mut App)) {
        let Err(p) = specular_core::isolate::run_reported(format_args!("UI: {what}"), || f(self))
        else {
            return;
        };
        let now = Instant::now();
        let recent = RECENT.with_borrow_mut(|r| {
            r.push_back(now);
            while r.front().is_some_and(|t| now - *t > GIVE_UP_WINDOW) {
                r.pop_front();
            }
            r.len()
        });
        if recent >= GIVE_UP_AFTER {
            // Outside the boundary: the panic hook writes the crash report.
            panic!("the UI panicked {recent} times in {GIVE_UP_WINDOW:?}; last: {p}");
        }
        if self.gfx.is_none() {
            // Before the window exists (start-up in `resumed`): going on would leave a
            // process with no window and nothing to say why. Crash with the report instead.
            panic!("the UI panicked before its window was up: {p}");
        }
        // The callback stopped part way: draw again, so the window does not sit on a stale
        // frame waiting for an event that the skipped code would have asked for.
        let now = self.now();
        self.chrome.warning(
            t!("background-failed", reason = t!("error-reason-internal")),
            now,
        );
        self.redraw = true;
        if let Some(g) = &self.gfx {
            g.window.request_redraw();
        }
    }
}
