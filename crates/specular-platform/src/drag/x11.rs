//! Drag-out on X11: an XDND (version 5) source, in safe Rust over its own x11rb connection.
//!
//! The window that was clicked keeps its implicit pointer grab (winit owns it), so this side
//! cannot grab the pointer. Instead a short-lived thread polls `QueryPointer` (every 15 ms),
//! finds the XDND-aware window under the pointer, talks XDND to it (`XdndEnter`,
//! `XdndPosition`, `XdndDrop`/`XdndLeave`) and serves the `XdndSelection` conversion
//! (`text/uri-list`, `text/plain`) until the target says `XdndFinished`. Limitations: no drag
//! cursor (only the grabbing client may change it), and no drop onto Specular's own window.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt, CreateWindowAux, EventMask, KeyButMask,
    PropMode, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use super::DragError;

const XDND_VERSION: u32 = 5;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        XdndAware,
        XdndProxy,
        XdndSelection,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndActionCopy,
        TARGETS,
        UTF8_STRING,
        URI_LIST: b"text/uri-list",
        TEXT_PLAIN: b"text/plain",
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
    }
}

pub(super) struct Source {
    own: Window,
    busy: Arc<AtomicBool>,
    /// For [`Source::pointer`]: made on first use (None: no X server reachable).
    query: std::sync::OnceLock<Option<Box<RustConnection>>>,
}

impl Source {
    pub(super) fn new(own: u32) -> Source {
        Source {
            own,
            busy: Arc::new(AtomicBool::new(false)),
            query: std::sync::OnceLock::new(),
        }
    }

    /// The pointer relative to our window (physical px) and the modifier keys, read from the
    /// server: during a drag from another client, winit reports neither (XDND positions go to
    /// winit, which drops them, and the source holds the pointer grab).
    pub(super) fn pointer(&self) -> Option<super::DropPointer> {
        let c = self
            .query
            .get_or_init(|| RustConnection::connect(None).ok().map(|(c, _)| Box::new(c)))
            .as_ref()?;
        let r = c.query_pointer(self.own).ok()?.reply().ok()?;
        if !r.same_screen {
            return None;
        }
        let m = r.mask;
        Some(super::DropPointer {
            x: f64::from(r.win_x),
            y: f64::from(r.win_y),
            shift: m.contains(KeyButMask::SHIFT),
            control: m.contains(KeyButMask::CONTROL),
            alt: m.contains(KeyButMask::MOD1),
        })
    }

    pub(super) fn active(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    pub(super) fn start(&self, uris: Vec<u8>, text: Vec<u8>) -> Result<(), DragError> {
        if self.busy.swap(true, Ordering::AcqRel) {
            return Err(DragError::NotReady);
        }
        let (conn, screen) = match RustConnection::connect(None) {
            Ok(c) => c,
            Err(e) => {
                self.busy.store(false, Ordering::Release);
                return Err(DragError::Failed(e.to_string()));
            }
        };
        let own = self.own;
        let busy = self.busy.clone();
        let spawned = specular_thread::builder("x11-drag").spawn(move || {
            super::one_drag("x11-drag", &busy, || {
                if let Err(e) = run(&conn, screen, own, &uris, &text) {
                    specular_log::debug!(target: specular_log::UI, "drag-out: {e}");
                }
            });
        });
        spawned.map(drop).map_err(|e| {
            self.busy.store(false, Ordering::Release);
            DragError::Failed(e.to_string())
        })
    }
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The window under the pointer that speaks XDND: `(window, message destination, version)`.
fn find_target(c: &RustConnection, a: &Atoms, root: Window) -> Res<Option<(Window, Window, u32)>> {
    let mut w = c.query_pointer(root)?.reply()?.child;
    for _ in 0..16 {
        if w == NONE {
            return Ok(None);
        }
        let aware = c
            .get_property(false, w, a.XdndAware, AtomEnum::ATOM, 0, 1)?
            .reply()?;
        if let Some(v) = aware.value32().and_then(|mut i| i.next()) {
            let proxy = c
                .get_property(false, w, a.XdndProxy, AtomEnum::WINDOW, 0, 1)?
                .reply()?;
            let dest = proxy.value32().and_then(|mut i| i.next()).unwrap_or(w);
            return Ok(Some((w, dest, v.min(XDND_VERSION))));
        }
        w = c.query_pointer(w)?.reply()?.child;
    }
    Ok(None)
}

fn message(
    c: &RustConnection,
    dest: Window,
    target: Window,
    kind: Atom,
    data: [u32; 5],
) -> Res<()> {
    let ev = ClientMessageEvent::new(32, target, kind, data);
    c.send_event(false, dest, EventMask::NO_EVENT, ev)?;
    Ok(())
}

/// Whether `w` is `own` or inside it (winit marks its own window XDND-aware to accept drops).
fn is_own(c: &RustConnection, mut w: Window, own: Window) -> bool {
    for _ in 0..16 {
        if w == own {
            return true;
        }
        match c.query_tree(w).ok().and_then(|q| q.reply().ok()) {
            Some(t) if t.parent != NONE && t.parent != t.root => w = t.parent,
            _ => return false,
        }
    }
    false
}

fn run(c: &RustConnection, screen: usize, own: Window, uris: &[u8], text: &[u8]) -> Res<()> {
    let root = c.setup().roots[screen].root;
    let a = Atoms::new(c)?.reply()?;
    let src = c.generate_id()?;
    c.create_window(
        0,
        src,
        root,
        -10,
        -10,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?;
    c.set_selection_owner(src, a.XdndSelection, CURRENT_TIME)?;
    c.flush()?;

    let types = [a.URI_LIST, a.TEXT_PLAIN_UTF8, a.TEXT_PLAIN];
    // (target, destination, version) currently entered.
    let mut cur: Option<(Window, Window, u32)> = None;
    let mut waiting_status = false;
    let mut accepted = false;
    let mut last_pos = (i16::MIN, i16::MIN);
    let started = Instant::now();
    let mut dropped_at: Option<Instant> = None;

    loop {
        while let Some(ev) = c.poll_for_event()? {
            match ev {
                Event::ClientMessage(m) if m.type_ == a.XdndStatus => {
                    let d = m.data.as_data32();
                    if cur.is_some_and(|t| t.0 == d[0]) {
                        waiting_status = false;
                        accepted = d[1] & 1 == 1;
                    }
                }
                Event::ClientMessage(m) if m.type_ == a.XdndFinished => {
                    if dropped_at.is_some() {
                        c.destroy_window(src)?;
                        c.flush()?;
                        return Ok(());
                    }
                }
                Event::SelectionRequest(r) => serve(c, &a, &r, &types, uris, text)?,
                _ => {}
            }
        }
        if let Some(t) = dropped_at {
            // Keep serving the selection for a while; then give up quietly.
            if t.elapsed() > Duration::from_secs(10) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        let p = c.query_pointer(root)?.reply()?;
        let held = p.mask.contains(KeyButMask::BUTTON1);
        let target = find_target(c, &a, root)?.filter(|t| !is_own(c, t.0, own));

        if !held || started.elapsed() > Duration::from_secs(120) {
            match cur {
                Some((t, dest, _)) if accepted && !held => {
                    message(c, dest, t, a.XdndDrop, [src, 0, CURRENT_TIME, 0, 0])?;
                    dropped_at = Some(Instant::now());
                }
                Some((t, dest, _)) => {
                    message(c, dest, t, a.XdndLeave, [src, 0, 0, 0, 0])?;
                    break;
                }
                None => break,
            }
            c.flush()?;
            continue;
        }

        if target.map(|t| t.0) != cur.map(|t| t.0) {
            if let Some((t, dest, _)) = cur.take() {
                message(c, dest, t, a.XdndLeave, [src, 0, 0, 0, 0])?;
            }
            if let Some((t, dest, v)) = target {
                message(
                    c,
                    dest,
                    t,
                    a.XdndEnter,
                    [src, v << 24, types[0], types[1], types[2]],
                )?;
                cur = target;
                waiting_status = false;
                accepted = false;
                last_pos = (i16::MIN, i16::MIN);
            }
        }
        if let Some((t, dest, _)) = cur
            && !waiting_status
            && (p.root_x, p.root_y) != last_pos
        {
            let xy = (u32::from(p.root_x as u16) << 16) | u32::from(p.root_y as u16);
            message(
                c,
                dest,
                t,
                a.XdndPosition,
                [src, 0, xy, CURRENT_TIME, a.XdndActionCopy],
            )?;
            waiting_status = true;
            last_pos = (p.root_x, p.root_y);
        }
        c.flush()?;
        std::thread::sleep(Duration::from_millis(15));
    }
    c.destroy_window(src)?;
    c.flush()?;
    Ok(())
}

/// Answers a conversion of `XdndSelection`.
fn serve(
    c: &RustConnection,
    a: &Atoms,
    r: &SelectionRequestEvent,
    types: &[Atom],
    uris: &[u8],
    text: &[u8],
) -> Res<()> {
    let prop = if r.property == NONE {
        r.target
    } else {
        r.property
    };
    let ok = if r.selection != a.XdndSelection {
        false
    } else if r.target == a.TARGETS {
        let mut list = types.to_vec();
        list.extend([a.TARGETS, a.UTF8_STRING]);
        c.change_property32(PropMode::REPLACE, r.requestor, prop, AtomEnum::ATOM, &list)?;
        true
    } else if r.target == a.URI_LIST {
        c.change_property8(PropMode::REPLACE, r.requestor, prop, r.target, uris)?;
        true
    } else if [a.TEXT_PLAIN, a.TEXT_PLAIN_UTF8, a.UTF8_STRING].contains(&r.target) {
        c.change_property8(PropMode::REPLACE, r.requestor, prop, r.target, text)?;
        true
    } else {
        false
    };
    let ev = SelectionNotifyEvent {
        response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: r.time,
        requestor: r.requestor,
        selection: r.selection,
        target: r.target,
        property: if ok { prop } else { NONE },
    };
    c.send_event(false, r.requestor, EventMask::NO_EVENT, ev)?;
    c.flush()?;
    Ok(())
}
