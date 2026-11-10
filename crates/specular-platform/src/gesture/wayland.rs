//! Pinch on Wayland: `zwp_pointer_gestures_v1` on a second event queue over winit's
//! `wl_display`.
//!
//! The arrangement is the drag-out source's (`drag/wayland.rs`, whose `foreign_connection` and
//! `foreign_surface` this reuses, UNSAFE.md row 22): our own `wl_seat`, `wl_pointer` and pinch
//! object on our own queue, dispatched by a background thread. The compositor sends the
//! gesture to every pointer of the client, so the pinch events also tell which surface they are
//! over; only winit's window surface counts.
//!
//! Dropping the [`Listener`] stops and joins the thread; it must be dropped before winit's event
//! loop (which frees the display), exactly like the drag source.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use wayland_client::backend::ObjectId;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_pinch_v1::{
    self, ZwpPointerGesturePinchV1,
};
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gestures_v1::{
    self, ZwpPointerGesturesV1,
};

use super::{GesturePhase, Pinch, PinchScale, Sink};
use crate::drag::wayland::{foreign_connection, foreign_surface};

pub(super) struct Listener {
    conn: Connection,
    qh: Arc<Mutex<Option<QueueHandle<State>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct State {
    sink: Sink,
    /// winit's window surface.
    surface: ObjectId,
    seat: Option<WlSeat>,
    pointer: Option<WlPointer>,
    gestures: Option<ZwpPointerGesturesV1>,
    pinch: Option<ZwpPointerGesturePinchV1>,
    /// A pinch over our surface is running.
    active: bool,
    scale: PinchScale,
}

impl Listener {
    /// `display` and `surface` are the `wl_display*` and `wl_surface*` from winit's window
    /// handle.
    pub(super) fn new(
        display: *mut std::ffi::c_void,
        surface: *mut std::ffi::c_void,
        sink: Sink,
    ) -> Result<Listener, String> {
        let conn = foreign_connection(display);
        let surface = foreign_surface(surface).map_err(|e| e.to_string())?;
        let qh = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, q, st) = (conn.clone(), qh.clone(), stop.clone());
        let thread = specular_thread::builder("wl-gesture")
            .spawn(move || {
                // Each run binds anew on a queue of its own.
                super::supervised("wl-gesture", &st, sink, |sink| {
                    let state = State {
                        sink,
                        surface: surface.clone(),
                        seat: None,
                        pointer: None,
                        gestures: None,
                        pinch: None,
                        active: false,
                        scale: PinchScale::default(),
                    };
                    run(c.clone(), state, q.clone(), &st);
                });
            })
            .map_err(|e| e.to_string())?;
        Ok(Listener {
            conn,
            qh,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Listener {
    /// Stops the queue thread and waits for it (see the module docs).
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        {
            let guard = self.qh.lock().unwrap_or_else(|e| e.into_inner());
            // A `wl_display.sync` on the thread's queue wakes its blocking dispatch. Without a
            // queue handle the thread has not reached its loop (it checks `stop` first) or has
            // already left it.
            if let Some(qh) = guard.as_ref() {
                self.conn.display().sync(qh, ());
            }
        }
        let _ = self.conn.flush();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    conn: Connection,
    mut state: State,
    qh_out: Arc<Mutex<Option<QueueHandle<State>>>>,
    stop: &AtomicBool,
) {
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&conn) else {
        specular_log::warn!(target: specular_log::INPUT, "touchpad gestures: no Wayland registry");
        return;
    };
    let qh = queue.handle();
    let gestures = globals
        .bind::<ZwpPointerGesturesV1, _, _>(&qh, 1..=3, ())
        .ok();
    let seat = globals.bind::<WlSeat, _, _>(&qh, 1..=5, ()).ok();
    let (Some(gestures), Some(seat)) = (gestures, seat) else {
        specular_log::info!(target: specular_log::INPUT, "touchpad gestures: compositor has no zwp_pointer_gestures_v1 or seat");
        return;
    };
    specular_log::debug!(target: specular_log::INPUT, "touchpad gestures: zwp_pointer_gestures_v1 v{}", gestures.version());
    state.gestures = Some(gestures);
    state.seat = Some(seat);
    {
        let mut g = qh_out.lock().unwrap_or_else(|e| e.into_inner());
        if stop.load(Ordering::Acquire) {
            return;
        }
        *g = Some(qh.clone());
    }
    while !stop.load(Ordering::Acquire) && queue.blocking_dispatch(&mut state).is_ok() {}
    // Shutting down: destroy what this queue created, then a round trip so nothing (such as
    // the wake-up callback) is left to arrive for the queue once it is gone.
    *qh_out.lock().unwrap_or_else(|e| e.into_inner()) = None;
    state.release_pointer();
    if let Some(g) = state.gestures.take()
        && g.version() >= 2
    {
        g.release();
    }
    if let Some(s) = state.seat.take()
        && s.version() >= 5
    {
        s.release();
    }
    let _ = queue.roundtrip(&mut state);
}

impl State {
    fn release_pointer(&mut self) {
        if let Some(p) = self.pinch.take() {
            p.destroy();
        }
        if let Some(p) = self.pointer.take()
            && p.version() >= 3
        {
            p.release();
        }
        self.end(true);
    }

    /// Ends a running pinch (a cancel when the pointer goes away mid-gesture).
    fn end(&mut self, cancelled: bool) {
        if std::mem::take(&mut self.active) {
            (self.sink)(Pinch {
                phase: if cancelled {
                    GesturePhase::Cancel
                } else {
                    GesturePhase::End
                },
                delta: 0.0,
                rotation: 0.0,
            });
        }
    }
}

impl Dispatch<WlCallback, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlCallback,
        _: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(c),
        } = event
        {
            let has = c.contains(wl_seat::Capability::Pointer);
            if has && state.pointer.is_none() {
                let pointer = seat.get_pointer(qh, ());
                if let Some(g) = &state.gestures {
                    state.pinch = Some(g.get_pinch_gesture(&pointer, qh, ()));
                }
                state.pointer = Some(pointer);
            } else if !has {
                state.release_pointer();
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlPointer,
        _: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpPointerGesturesV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpPointerGesturesV1,
        _: zwp_pointer_gestures_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpPointerGesturePinchV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwpPointerGesturePinchV1,
        event: zwp_pointer_gesture_pinch_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_pointer_gesture_pinch_v1::Event::Begin {
                surface, fingers, ..
            } => {
                state.end(true);
                if surface.id() != state.surface {
                    return;
                }
                specular_log::trace!(target: specular_log::INPUT, "wayland pinch begin, {fingers} fingers");
                specular_log::service::trip();
                state.active = true;
                state.scale.begin();
                (state.sink)(Pinch {
                    phase: GesturePhase::Begin,
                    delta: 0.0,
                    rotation: 0.0,
                });
            }
            zwp_pointer_gesture_pinch_v1::Event::Update {
                scale, rotation, ..
            } if state.active => {
                let delta = state.scale.update(scale);
                (state.sink)(Pinch {
                    phase: GesturePhase::Update,
                    delta,
                    // The protocol turns clockwise; winit counter-clockwise.
                    rotation: if rotation.is_finite() {
                        -rotation as f32
                    } else {
                        0.0
                    },
                });
            }
            zwp_pointer_gesture_pinch_v1::Event::End { cancelled, .. } => state.end(cancelled != 0),
            _ => {}
        }
    }
}
