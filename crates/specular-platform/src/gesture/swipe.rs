//! Two-finger "swipe between pages" (macOS): AppKit's own swipe tracking, as Safari and Preview
//! use it.
//!
//! A local event monitor watches scroll events. When a touchpad scroll gesture starts sideways
//! and the application offers a page that way, the gesture is handed to
//! `-[NSEvent trackSwipeEventWithOptions:dampenAmountThresholdMin:max:usingHandler:]`. AppKit then
//! takes the rest of the gesture and its momentum (they never reach winit), decides whether the
//! swipe completed or was cancelled, and honours the user's "Swipe between pages" setting
//! (`+[NSEvent isSwipeTrackingFromScrollEventsEnabled]`). As in WebKit, the gesture's direction is
//! read from its first event with movement: the `Began` event often has none.
//!
//! Elsewhere nothing listens (`PageSwipes::active` is false).

/// Which way the fingers moved in a completed swipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeDirection {
    Left,
    Right,
}

/// Callback for completed swipes; called on the main thread, inside AppKit's event dispatch.
pub type SwipeSink = Box<dyn Fn(SwipeDirection)>;

/// Page swipes for the application's windows. Create it on the main thread once a window
/// exists; dropping it removes the monitor.
pub struct PageSwipes {
    #[cfg(target_os = "macos")]
    imp: Option<macos::Monitor>,
}

impl std::fmt::Debug for PageSwipes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.active() {
            "PageSwipes(AppKit swipe tracking)"
        } else {
            "PageSwipes(none)"
        })
    }
}

impl PageSwipes {
    /// Starts watching for swipes, handing completed ones to `sink`. Never fails: where the
    /// system has no page swipes, nothing listens.
    pub fn new(sink: SwipeSink) -> PageSwipes {
        #[cfg(target_os = "macos")]
        {
            PageSwipes {
                imp: macos::Monitor::new(sink),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = sink;
            PageSwipes {}
        }
    }

    pub fn active(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.imp.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// Which swipes a gesture starting now may become: fingers moving `left`, `right`. Neither
    /// leaves every scroll to the application.
    pub fn offer(&self, left: bool, right: bool) {
        #[cfg(target_os = "macos")]
        if let Some(m) = &self.imp {
            m.offer.set((left, right));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (left, right);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::Cell;
    use std::ptr::NonNull;
    use std::rc::Rc;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool};
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventPhase, NSEventSwipeTrackingOptions};

    use super::{SwipeDirection, SwipeSink};

    pub(super) struct Monitor {
        token: Retained<AnyObject>,
        pub(super) offer: Rc<Cell<(bool, bool)>>,
    }

    impl Monitor {
        pub(super) fn new(sink: SwipeSink) -> Option<Monitor> {
            let offer = Rc::new(Cell::new((false, false)));
            let sink: Rc<dyn Fn(SwipeDirection)> = Rc::from(sink);
            // A scroll gesture began and its direction is not known yet.
            let pending = Cell::new(false);
            let o = offer.clone();
            let block = RcBlock::new(move |ev: NonNull<NSEvent>| -> *mut NSEvent {
                if scroll(ev, &pending, o.get(), &sink) {
                    std::ptr::null_mut()
                } else {
                    ev.as_ptr()
                }
            });
            let token = add_monitor(&block)?;
            Some(Monitor { token, offer })
        }
    }

    impl Drop for Monitor {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: `token` is the object `addLocalMonitorForEventsMatchingMask:handler:`
            // returned, removed exactly once.
            unsafe { NSEvent::removeMonitor(&self.token) };
        }
    }

    #[allow(unsafe_code)]
    fn add_monitor(
        block: &block2::DynBlock<dyn Fn(NonNull<NSEvent>) -> *mut NSEvent>,
    ) -> Option<Retained<AnyObject>> {
        // SAFETY: the block takes the event AppKit is dispatching and returns it, or null to
        // swallow it, as the method requires; AppKit copies the block. Called on the main
        // thread, where the application's events are dispatched.
        unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::ScrollWheel, block)
        }
    }

    #[allow(unsafe_code)]
    fn event_ref<'a>(ev: NonNull<NSEvent>) -> &'a NSEvent {
        // SAFETY: AppKit passes the event it is dispatching, alive for the monitor's call; the
        // reference does not outlive it.
        unsafe { ev.as_ref() }
    }

    /// One scroll event. True when it started a swipe and must not reach the window.
    fn scroll(
        ev: NonNull<NSEvent>,
        pending: &Cell<bool>,
        (left, right): (bool, bool),
        sink: &Rc<dyn Fn(SwipeDirection)>,
    ) -> bool {
        let event = event_ref(ev);
        let phase = event.phase();
        if phase == NSEventPhase::Began {
            pending.set(true);
        }
        if !pending.get() {
            return false;
        }
        if phase != NSEventPhase::Began && phase != NSEventPhase::Changed {
            pending.set(false);
            return false;
        }
        let (dx, dy) = (event.scrollingDeltaX(), event.scrollingDeltaY());
        if dx == 0.0 && dy == 0.0 {
            return false;
        }
        pending.set(false);
        if dx.abs() <= dy.abs()
            || !(left || right)
            || !event.hasPreciseScrollingDeltas()
            || !NSEvent::isSwipeTrackingFromScrollEventsEnabled()
        {
            return false;
        }
        let sink = sink.clone();
        // The amount runs from 0 towards +1 as the fingers move right, -1 left. `Ended` means
        // the swipe completes (`Cancelled`: it springs back).
        let handler = RcBlock::new(
            move |amount: f64, phase: NSEventPhase, _complete: Bool, _stop: NonNull<Bool>| {
                if phase == NSEventPhase::Ended && amount != 0.0 {
                    sink(if amount > 0.0 {
                        SwipeDirection::Right
                    } else {
                        SwipeDirection::Left
                    });
                }
            },
        );
        event.trackSwipeEventWithOptions_dampenAmountThresholdMin_max_usingHandler(
            NSEventSwipeTrackingOptions::LockDirection
                | NSEventSwipeTrackingOptions::ClampGestureAmount,
            if left { -1.0 } else { 0.0 },
            if right { 1.0 } else { 0.0 },
            &handler,
        );
        true
    }
}
