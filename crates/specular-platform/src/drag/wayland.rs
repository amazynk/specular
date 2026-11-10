//! Drag-out on Wayland: `wl_data_device.start_drag` with a `wl_data_source` offering
//! `text/uri-list`.
//!
//! winit has no drag source and does not expose the input serial `start_drag` needs, so this
//! module opens its own event queue on winit's `wl_display` (one connection, one client: the
//! compositor accepts winit's `wl_surface` as the drag origin), binds its own `wl_seat` and
//! `wl_pointer` to learn the serial of each button press, and creates its own data device.
//! A background thread dispatches the queue: it answers the receiving application's `send`
//! requests by writing the URI list into the pipe the compositor hands over.
//!
//! The display belongs to winit, which disconnects (frees) it when its event loop is dropped.
//! Dropping the [`Source`] stops and joins that thread, so it must be dropped first (the app does
//! it in `ApplicationHandler::exiting`): a thread left in `blocking_dispatch` would read a
//! closed fd and freed memory (`Io error: Bad file descriptor`, then SIGSEGV at exit).

use std::io::Write;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::WlDataOffer;
use wayland_client::protocol::wl_data_source::{self, WlDataSource};
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, event_created_child};

use super::DragError;

/// Mime types offered, in order of preference.
const URI_LIST: &str = "text/uri-list";
const TEXT: &str = "text/plain;charset=utf-8";

/// What the UI thread needs to start a drag.
struct Ready {
    qh: QueueHandle<State>,
    manager: WlDataDeviceManager,
    device: WlDataDevice,
}

pub(super) struct Source {
    conn: Connection,
    surface: WlSurface,
    serial: Arc<AtomicU32>,
    ready: Arc<Mutex<Option<Ready>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct State {
    serial: Arc<AtomicU32>,
    seat: Option<WlSeat>,
    pointer: Option<WlPointer>,
}

/// The data a source hands out.
struct Payload {
    uris: Arc<[u8]>,
    text: Arc<[u8]>,
}

impl Source {
    /// `display` and `surface` are the `wl_display*` and `wl_surface*` from winit's window
    /// handle.
    pub(super) fn new(
        display: *mut std::ffi::c_void,
        surface: *mut std::ffi::c_void,
    ) -> Result<Source, DragError> {
        let conn = foreign_connection(display);
        let id = foreign_surface(surface)?;
        let surface =
            WlSurface::from_id(&conn, id).map_err(|e| DragError::Failed(e.to_string()))?;
        let serial = Arc::new(AtomicU32::new(0));
        let ready = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, s, r, st) = (conn.clone(), serial.clone(), ready.clone(), stop.clone());
        let thread = specular_thread::builder("wl-drag")
            .spawn(move || {
                // A panic leaves no queue to start drags on: `ready` is cleared (drags say
                // "not ready" instead of vanishing) and the queue is set up again.
                specular_log::service::supervise(
                    "wl-drag",
                    specular_log::service::Policy::RESTART,
                    || st.load(Ordering::Acquire),
                    || run(c.clone(), s.clone(), r.clone(), &st),
                    |_| {
                        *r.lock().unwrap_or_else(|e| e.into_inner()) = None;
                        s.store(0, Ordering::Release);
                    },
                );
            })
            .map_err(|e| DragError::Failed(e.to_string()))?;
        Ok(Source {
            conn,
            surface,
            serial,
            ready,
            stop,
            thread: Some(thread),
        })
    }

    pub(super) fn start(&self, uris: Vec<u8>, text: Vec<u8>) -> Result<(), DragError> {
        let serial = self.serial.load(Ordering::Acquire);
        let guard = self.ready.lock().unwrap_or_else(|e| e.into_inner());
        let Some(r) = guard.as_ref() else {
            return Err(DragError::NotReady);
        };
        if serial == 0 {
            return Err(DragError::NotReady);
        }
        let payload = Payload {
            uris: uris.into(),
            text: text.into(),
        };
        let source = r.manager.create_data_source(&r.qh, payload);
        source.offer(URI_LIST.into());
        source.offer(TEXT.into());
        if source.version() >= 3 {
            source.set_actions(DndAction::Copy);
        }
        r.device
            .start_drag(Some(&source), &self.surface, None, serial);
        self.conn
            .flush()
            .map_err(|e| DragError::Failed(e.to_string()))
    }
}

impl Drop for Source {
    /// Stops the queue thread and waits for it (see the module docs).
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        {
            let guard = self.ready.lock().unwrap_or_else(|e| e.into_inner());
            // A `wl_display.sync` on the thread's queue wakes its blocking dispatch. Without
            // `Ready` the thread has either not reached its loop (it checks `stop` first) or
            // has already left it.
            if let Some(r) = guard.as_ref() {
                self.conn.display().sync(&r.qh, ());
            }
        }
        let _ = self.conn.flush();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Also used by the pinch listener (`gesture/wayland.rs`), under the same rules.
#[allow(unsafe_code)]
pub(crate) fn foreign_connection(display: *mut std::ffi::c_void) -> Connection {
    // SAFETY: `display` is the live `wl_display*` of winit's connection (from the window's
    // display handle). winit keeps it open for the life of its event loop; `Source` and the
    // gesture `Listener` (each owns its connection and joins the only thread using it) are
    // dropped before the event loop (module docs). This backend does not own the display and never disconnects it.
    let backend = unsafe { Backend::from_foreign_display(display.cast()) };
    Connection::from_backend(backend)
}

#[allow(unsafe_code)]
pub(crate) fn foreign_surface(surface: *mut std::ffi::c_void) -> Result<ObjectId, DragError> {
    // SAFETY: `surface` is the live `wl_surface*` proxy of winit's window, created on the same
    // display; `from_ptr` checks that its interface really is `wl_surface`. The window (and the
    // proxy) outlives every drag, which only starts from that window's input events; the
    // gesture listener only compares the id with the surfaces in its pinch events.
    unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.cast()) }
        .map_err(|e| DragError::Failed(e.to_string()))
}

fn run(
    conn: Connection,
    serial: Arc<AtomicU32>,
    ready: Arc<Mutex<Option<Ready>>>,
    stop: &AtomicBool,
) {
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&conn) else {
        specular_log::warn!(target: specular_log::UI, "drag-out: no Wayland registry");
        return;
    };
    let qh = queue.handle();
    let seat = globals.bind::<WlSeat, _, _>(&qh, 1..=5, ()).ok();
    let manager = globals
        .bind::<WlDataDeviceManager, _, _>(&qh, 1..=3, ())
        .ok();
    let (Some(seat), Some(manager)) = (seat, manager) else {
        specular_log::info!(target: specular_log::UI, "drag-out: compositor has no seat or data device manager");
        return;
    };
    let device = manager.get_data_device(&seat, &qh, ());
    *ready.lock().unwrap_or_else(|e| e.into_inner()) = Some(Ready {
        qh: qh.clone(),
        manager,
        device,
    });
    let mut state = State {
        serial,
        seat: Some(seat),
        pointer: None,
    };
    while !stop.load(Ordering::Acquire) {
        specular_log::service::trip();
        if queue.blocking_dispatch(&mut state).is_err() {
            break;
        }
    }
    // Shutting down: destroy what this queue created, then a round trip so nothing (such as
    // the wake-up callback) is left to arrive for the queue once it is gone.
    let r = ready.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(r) = r
        && r.device.version() >= 2
    {
        r.device.release();
    }
    if let Some(p) = state.pointer.take()
        && p.version() >= 3
    {
        p.release();
    }
    if let Some(s) = state.seat.take()
        && s.version() >= 5
    {
        s.release();
    }
    let _ = queue.roundtrip(&mut state);
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
                state.pointer = Some(seat.get_pointer(qh, ()));
            } else if !has
                && let Some(p) = state.pointer.take()
                && p.version() >= 3
            {
                p.release();
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            serial,
            state: WEnum::Value(wl_pointer::ButtonState::Pressed),
            ..
        } = event
        {
            state.serial.store(serial, Ordering::Release);
        }
    }
}

impl Dispatch<WlDataDeviceManager, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: <WlDataDeviceManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // This device only drags out; offers it is told about (clipboard, drags over the
        // window, which winit's own device handles) are released at once.
        match event {
            wl_data_device::Event::Selection { id: Some(o) }
            | wl_data_device::Event::Enter { id: Some(o), .. } => {
                o.destroy();
            }
            _ => {}
        }
    }

    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}

impl Dispatch<WlDataOffer, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        _: <WlDataOffer as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlDataSource, Payload> for State {
    fn event(
        _: &mut Self,
        source: &WlDataSource,
        event: wl_data_source::Event,
        p: &Payload,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { mime_type, fd } => send(&mime_type, fd, p),
            wl_data_source::Event::Cancelled | wl_data_source::Event::DndFinished => {
                source.destroy()
            }
            _ => {}
        }
    }
}

/// Writes the data on its own thread: a receiver that reads slowly (or never) must not stall
/// this queue.
fn send(mime: &str, fd: OwnedFd, p: &Payload) {
    let data = if mime == URI_LIST {
        p.uris.clone()
    } else {
        p.text.clone()
    };
    send_on_thread(data, fd);
}

/// One transfer behind a boundary: a panic or a failed write costs that transfer and is
/// logged; a thread that cannot start is logged too (the receiver sees an empty drop).
fn send_on_thread(data: Arc<[u8]>, fd: OwnedFd) -> Option<JoinHandle<()>> {
    let spawned = specular_thread::builder(SEND_THREAD).spawn(move || {
        let mut job = Some((data, fd));
        specular_log::service::supervise(
            SEND_THREAD,
            specular_log::service::Policy::Stop,
            || false,
            || {
                specular_log::service::trip();
                if let Some((data, fd)) = job.take() {
                    write_to_receiver(&data, fd);
                }
            },
            |_| {},
        );
    });
    match spawned {
        Ok(t) => Some(t),
        Err(e) => {
            specular_log::warn!(target: specular_log::UI, "drag-out: could not start a transfer: {e}");
            None
        }
    }
}

const SEND_THREAD: &str = "wl-drag-send";

fn write_to_receiver(data: &[u8], fd: OwnedFd) {
    let mut f = std::fs::File::from(fd);
    match f.write_all(data) {
        Ok(()) => {}
        // The receiver took what it wanted and closed its end: not a failure.
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
            specular_log::debug!(target: specular_log::UI, "drag-out: receiver closed early: {e}");
        }
        Err(e) => {
            specular_log::warn!(target: specular_log::UI, "drag-out: write to receiver failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    /// The send threads' panic arming is process-wide: the tests that start one take turns,
    /// or another test's transfer can take the armed panic.
    static SENDS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A transfer whose thread panics ends there (logged, counted); the next one still
    /// delivers its data.
    #[test]
    fn a_panicking_transfer_costs_only_that_transfer() {
        let _turn = SENDS.lock().unwrap_or_else(|e| e.into_inner());
        let (mut r1, w1) = std::io::pipe().unwrap();
        specular_log::service::arm(SEND_THREAD, 1);
        send_on_thread(Arc::from(&b"lost"[..]), w1.into())
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(specular_log::service::deaths(SEND_THREAD), 1);
        let mut got = Vec::new();
        r1.read_to_end(&mut got).unwrap();
        assert!(
            got.is_empty(),
            "the panicking transfer's fd is closed, not leaked"
        );

        let (mut r2, w2) = std::io::pipe().unwrap();
        send_on_thread(Arc::from(&b"file:///a.jpg\r\n"[..]), w2.into())
            .unwrap()
            .join()
            .unwrap();
        r2.read_to_end(&mut got).unwrap();
        assert_eq!(got, b"file:///a.jpg\r\n");
    }

    /// A receiver that closed its end is not a failure and does not panic.
    #[test]
    fn a_closed_receiver_is_not_a_panic() {
        let _turn = SENDS.lock().unwrap_or_else(|e| e.into_inner());
        let (r, w) = std::io::pipe().unwrap();
        drop(r);
        send_on_thread(Arc::from(&b"x"[..]), w.into())
            .unwrap()
            .join()
            .unwrap();
    }
}
