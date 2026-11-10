//! Which thread is the UI's, so work that must never run there can check.
//!
//! The application marks its event-loop thread once at start-up ([`mark_ui_thread`]). Code
//! that may block for an unbounded time — the Library's catalogue (SQLite: opening,
//! migrations, searches, the Local AI pack's tables) — calls [`assert_not_ui_thread`] at its
//! way in. In debug builds a call on the marked thread panics at once, naming the caller, so a
//! frame-blocking path fails its test instead of stuttering for a user; in release builds the
//! check compiles to nothing.

use std::cell::Cell;

thread_local! {
    static UI: Cell<bool> = const { Cell::new(false) };
}

/// Marks the calling thread as the UI thread (the event loop's). Call once, at start-up.
pub fn mark_ui_thread() {
    UI.with(|c| c.set(true));
}

/// Whether the calling thread was marked by [`mark_ui_thread`] (or a [`UiThreadScope`]).
pub fn is_ui_thread() -> bool {
    UI.with(Cell::get)
}

/// Debug builds: panics when called on the UI thread, with `what` and the caller's place.
/// Release builds: nothing.
#[track_caller]
#[inline(always)]
pub fn assert_not_ui_thread(what: &str) {
    if cfg!(debug_assertions) && is_ui_thread() {
        panic!(
            "{what} ran on the UI thread (at {}): it may block for seconds; move it to a worker",
            std::panic::Location::caller()
        );
    }
}

/// Marks the calling thread as the UI thread until dropped (tests that act as the UI thread).
pub struct UiThreadScope {
    was: bool,
}

impl UiThreadScope {
    pub fn enter() -> UiThreadScope {
        let was = is_ui_thread();
        mark_ui_thread();
        UiThreadScope { was }
    }
}

impl Drop for UiThreadScope {
    fn drop(&mut self) {
        let was = self.was;
        UI.with(|c| c.set(was));
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    #[test]
    fn off_the_ui_thread_passes() {
        assert!(!is_ui_thread());
        assert_not_ui_thread("test");
        // Another thread is never the UI thread, even while this one is.
        let _ui = UiThreadScope::enter();
        std::thread::spawn(|| assert_not_ui_thread("worker"))
            .join()
            .expect("a worker is not the UI thread");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "catalogue ran on the UI thread")]
    fn on_the_ui_thread_trips() {
        let _ui = UiThreadScope::enter();
        assert_not_ui_thread("catalogue");
    }

    #[test]
    fn scope_restores() {
        {
            let _ui = UiThreadScope::enter();
            assert!(is_ui_thread());
        }
        assert!(!is_ui_thread());
    }
}
