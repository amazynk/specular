//! Pinch on X11: XInput 2.4 `GesturePinch{Begin,Update,End}` events, in safe Rust over our own
//! x11rb connection.
//!
//! winit 0.30 neither selects nor reports XI gestures, so this connection selects them on
//! winit's window (as with button presses, only one client may select gesture events on a
//! window; winit does not). A background thread waits for events; dropping the [`Listener`]
//! sends a client message to a small window of our own to wake it, then joins it. Unlike
//! Wayland nothing is shared with winit, so the drop order does not matter.
//!
//! Needs XI 2.4 (Xorg 21.1+ with the libinput driver, or XWayland 21.1+); older servers just
//! leave the listener off.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use x11rb::connection::{Connection, RequestConnection as _};
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{
    self, ConnectionExt as _, Device, EventMask as XiEventMask, GesturePinchBeginEvent,
    GesturePinchEventFlags, XIEventMask,
};
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, Window,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;

use super::{GesturePhase, Pinch, PinchScale, Sink};

/// XI 2.4 event mask bits (x11rb's `XIEventMask` has no names for them).
const PINCH_MASK: u32 = (1 << xinput::GESTURE_PINCH_BEGIN_EVENT)
    | (1 << xinput::GESTURE_PINCH_UPDATE_EVENT)
    | (1 << xinput::GESTURE_PINCH_END_EVENT);

pub(super) struct Listener {
    conn: Arc<RustConnection>,
    /// Our own input-only window, the target of the wake-up message.
    wake: Window,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl Listener {
    pub(super) fn new(window: Window, sink: Sink) -> Result<Listener, String> {
        Self::setup(window, sink).map_err(|e| e.to_string())
    }

    fn setup(window: Window, sink: Sink) -> Res<Listener> {
        let (conn, screen) = RustConnection::connect(None)?;
        if conn
            .extension_information(xinput::X11_EXTENSION_NAME)?
            .is_none()
        {
            return Err("the X server has no XInput".into());
        }
        let v = conn.xinput_xi_query_version(2, 4)?.reply()?;
        if (v.major_version, v.minor_version) < (2, 4) {
            return Err(format!(
                "the X server has XInput {}.{}; pinch needs 2.4",
                v.major_version, v.minor_version
            )
            .into());
        }
        let root = conn.setup().roots[screen].root;
        let wake = conn.generate_id()?;
        conn.create_window(
            0,
            wake,
            root,
            -1,
            -1,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new(),
        )?
        .check()?;
        conn.xinput_xi_select_events(
            window,
            &[XiEventMask {
                deviceid: u16::from(Device::ALL_MASTER),
                mask: vec![XIEventMask::from(PINCH_MASK)],
            }],
        )?
        .check()?;
        conn.flush()?;
        specular_log::debug!(target: specular_log::INPUT, "touchpad gestures: XInput {}.{} pinch on window {window:#x}", v.major_version, v.minor_version);
        let conn = Arc::new(conn);
        let stop = Arc::new(AtomicBool::new(false));
        let (c, st) = (conn.clone(), stop.clone());
        let thread = specular_thread::builder("x11-gesture").spawn(move || {
            super::supervised("x11-gesture", &st, sink, |sink| run(&c, window, sink, &st));
        })?;
        Ok(Listener {
            conn,
            wake,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let ev = ClientMessageEvent::new(32, self.wake, AtomEnum::INTEGER, [0u32; 5]);
        let _ = self
            .conn
            .send_event(false, self.wake, EventMask::NO_EVENT, ev);
        let _ = self.conn.flush();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = self.conn.destroy_window(self.wake);
        let _ = self.conn.flush();
    }
}

fn run(c: &RustConnection, window: Window, sink: Sink, stop: &AtomicBool) {
    let mut scale = PinchScale::default();
    let mut active = false;
    while !stop.load(Ordering::Acquire) {
        let ev = match c.wait_for_event() {
            Ok(ev) => ev,
            Err(e) => {
                specular_log::debug!(target: specular_log::INPUT, "touchpad gestures: X connection lost: {e}");
                break;
            }
        };
        specular_log::service::trip();
        if let Some(p) = translate(&ev, window, &mut scale, &mut active) {
            sink(p);
        }
    }
}

/// One XI event as a [`Pinch`], if it is a pinch over `window`.
fn translate(
    ev: &Event,
    window: Window,
    scale: &mut PinchScale,
    active: &mut bool,
) -> Option<Pinch> {
    let fp = |v: i32| f64::from(v) / 65536.0;
    let over = |e: &GesturePinchBeginEvent| e.event == window;
    match ev {
        Event::XinputGesturePinchBegin(e) if over(e) => {
            specular_log::trace!(target: specular_log::INPUT, "x11 pinch begin, {} fingers", e.detail);
            *active = true;
            scale.begin();
            Some(Pinch {
                phase: GesturePhase::Begin,
                delta: 0.0,
                rotation: 0.0,
            })
        }
        Event::XinputGesturePinchUpdate(e) if over(e) && *active => Some(Pinch {
            phase: GesturePhase::Update,
            delta: scale.update(fp(e.scale)),
            // libinput's angle grows clockwise; winit's counter-clockwise.
            rotation: -fp(e.delta_angle) as f32,
        }),
        Event::XinputGesturePinchEnd(e) if over(e) && std::mem::take(active) => Some(Pinch {
            phase: if e
                .flags
                .contains(GesturePinchEventFlags::GESTURE_PINCH_CANCELLED)
            {
                GesturePhase::Cancel
            } else {
                GesturePhase::End
            },
            delta: 0.0,
            rotation: 0.0,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: u16, scale: f64, flags: u32) -> GesturePinchBeginEvent {
        GesturePinchBeginEvent {
            response_type: 35,
            extension: 0,
            sequence: 0,
            length: 0,
            event_type: kind,
            deviceid: 2,
            time: 0,
            detail: 2,
            root: 1,
            event: 7,
            child: 0,
            root_x: 0,
            root_y: 0,
            event_x: 0,
            event_y: 0,
            delta_x: 0,
            delta_y: 0,
            delta_unaccel_x: 0,
            delta_unaccel_y: 0,
            scale: (scale * 65536.0) as i32,
            delta_angle: 2 << 16,
            sourceid: 2,
            mods: Default::default(),
            group: Default::default(),
            flags: flags.into(),
        }
    }

    #[test]
    fn pinch_sequence_becomes_relative_deltas() {
        let (mut s, mut a) = (PinchScale::default(), false);
        let seq = [
            Event::XinputGesturePinchBegin(ev(27, 1.0, 0)),
            Event::XinputGesturePinchUpdate(ev(28, 1.25, 0)),
            Event::XinputGesturePinchUpdate(ev(28, 1.5, 0)),
            Event::XinputGesturePinchEnd(ev(29, 1.5, 0)),
        ];
        let out: Vec<Pinch> = seq
            .iter()
            .filter_map(|e| translate(e, 7, &mut s, &mut a))
            .collect();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].phase, GesturePhase::Begin);
        assert!((out[1].delta - 0.25).abs() < 1e-4);
        assert!((out[2].delta - 0.2).abs() < 1e-4);
        assert_eq!(out[1].rotation, -2.0, "clockwise turned counter-clockwise");
        assert_eq!(out[3].phase, GesturePhase::End);
    }

    #[test]
    fn other_windows_and_stray_events_are_ignored() {
        let (mut s, mut a) = (PinchScale::default(), false);
        // An update or end without our begin.
        assert!(
            translate(
                &Event::XinputGesturePinchUpdate(ev(28, 1.2, 0)),
                7,
                &mut s,
                &mut a
            )
            .is_none()
        );
        assert!(
            translate(
                &Event::XinputGesturePinchEnd(ev(29, 1.2, 0)),
                7,
                &mut s,
                &mut a
            )
            .is_none()
        );
        // Another window.
        assert!(
            translate(
                &Event::XinputGesturePinchBegin(ev(27, 1.0, 0)),
                8,
                &mut s,
                &mut a
            )
            .is_none()
        );
        // Cancelled.
        translate(
            &Event::XinputGesturePinchBegin(ev(27, 1.0, 0)),
            7,
            &mut s,
            &mut a,
        );
        let end = translate(
            &Event::XinputGesturePinchEnd(ev(29, 1.0, 1)),
            7,
            &mut s,
            &mut a,
        );
        assert_eq!(end.map(|p| p.phase), Some(GesturePhase::Cancel));
    }
}
