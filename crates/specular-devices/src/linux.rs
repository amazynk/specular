//! Linux: devices from sysfs, read from `/dev/input` by one blocking thread each, and a
//! SpaceMouse through spacenavd when its socket is there.
//!
//! Nothing here needs root: a node opens when the user's permissions allow (systemd gives
//! the person at the seat gamepads and joysticks through `uaccess`; other nodes belong to
//! the `input` group). One that does not is a [`Problem::NoPermission`] naming the node's
//! group. `/dev/input` is watched, so devices plugged in later are picked up.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::evdev::{self, InputEvent, PadReader, PuckReader};
use crate::joydev::{self, JsEvent, JsReader};
use crate::spnav::{self, Packet};
use crate::sysfs::{self, Found};
use crate::{DeviceId, DeviceInfo, Event, Kind, Problem, Sink, Via};

/// Where things are (tests point these at a scratch folder).
#[derive(Clone, Debug)]
pub struct Roots {
    /// `/sys/class/input`.
    pub class: PathBuf,
    /// `/dev/input`.
    pub dev: PathBuf,
    /// spacenavd's socket.
    pub spnav: PathBuf,
    /// `/etc/group`, to name a node's group.
    pub groups: PathBuf,
    /// Watch `dev` for devices coming and going.
    pub watch: bool,
}

impl Default for Roots {
    fn default() -> Roots {
        Roots {
            class: "/sys/class/input".into(),
            dev: "/dev/input".into(),
            spnav: spnav::SOCKET.into(),
            groups: "/etc/group".into(),
            watch: true,
        }
    }
}

struct Shared {
    roots: Roots,
    sink: Sink,
    stop: AtomicBool,
    next: AtomicU32,
    /// Nodes being read (and the spacenavd socket), by path.
    open: Mutex<HashMap<PathBuf, DeviceId>>,
    /// Nodes already reported as unreadable (reported again only after a rescan by hand).
    refused: Mutex<HashSet<PathBuf>>,
    /// spacenavd's connection, to shut it down on stop.
    spnav: Mutex<Option<UnixStream>>,
}

pub(crate) struct Backend {
    shared: Arc<Shared>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl Backend {
    pub(crate) fn start(roots: Roots, sink: Sink) -> Backend {
        let watch = roots.watch;
        let shared = Arc::new(Shared {
            roots,
            sink,
            stop: AtomicBool::new(false),
            next: AtomicU32::new(1),
            open: Mutex::new(HashMap::new()),
            refused: Mutex::new(HashSet::new()),
            spnav: Mutex::new(None),
        });
        scan(&shared);
        let watcher = watch.then(|| watcher(&shared)).flatten();
        Backend {
            shared,
            _watcher: watcher,
        }
    }

    pub(crate) fn rescan(&self) {
        if let Ok(mut r) = self.shared.refused.lock() {
            r.clear();
        }
        scan(&self.shared);
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Ok(mut s) = self.shared.spnav.lock()
            && let Some(s) = s.take()
        {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
}

/// Watches `/dev/input`: a node appearing or going looks again (after a moment, for udev
/// to set its permissions).
fn watcher(shared: &Arc<Shared>) -> Option<notify::RecommendedWatcher> {
    use notify::{EventKind, RecursiveMode, Watcher};
    let weak = Arc::downgrade(shared);
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(ev) = res else {
            return;
        };
        if !matches!(ev.kind, EventKind::Create(_) | EventKind::Remove(_)) {
            return;
        }
        if let Some(s) = weak.upgrade()
            && !s.stop.load(Ordering::SeqCst)
        {
            std::thread::sleep(Duration::from_millis(400));
            scan(&s);
        }
    })
    .ok()?;
    w.watch(&shared.roots.dev, RecursiveMode::NonRecursive)
        .ok()?;
    Some(w)
}

fn emit(s: &Shared, e: Event) {
    (s.sink)(e);
}

fn scan(s: &Arc<Shared>) {
    if s.stop.load(Ordering::SeqCst) {
        return;
    }
    let spnav = connect_spnav(s);
    for f in sysfs::scan(&s.roots.class, &s.roots.dev) {
        let (path, via) = match f.kind {
            // spacenavd holds the SpaceMouse; it is read through the daemon.
            Kind::SpaceMouse if spnav => continue,
            Kind::SpaceMouse => match &f.event {
                Some(p) => (p.clone(), Via::Evdev),
                None => continue,
            },
            Kind::Gamepad | Kind::Joystick => match (&f.js, &f.event) {
                (Some(js), _) => (js.clone(), Via::Joydev),
                (None, Some(ev)) => (ev.clone(), Via::Evdev),
                _ => continue,
            },
        };
        open_node(s, f, path, via);
    }
}

/// The name of group `gid` in an `/etc/group`-style file.
pub(crate) fn group_name(groups: &Path, gid: u32) -> Option<String> {
    let text = std::fs::read_to_string(groups).ok()?;
    text.lines().find_map(|l| {
        let mut it = l.split(':');
        let name = it.next()?;
        let _pw = it.next()?;
        let id: u32 = it.next()?.trim().parse().ok()?;
        (id == gid && !name.is_empty()).then(|| name.to_owned())
    })
}

fn open_node(s: &Arc<Shared>, f: Found, path: PathBuf, via: Via) {
    let Ok(mut open) = s.open.lock() else {
        return;
    };
    if open.contains_key(&path) {
        return;
    }
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            drop(open);
            let first = s
                .refused
                .lock()
                .map(|mut r| r.insert(path.clone()))
                .unwrap_or(false);
            if first {
                let group = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| group_name(&s.roots.groups, m.gid()));
                emit(
                    s,
                    Event::Problem(Problem::NoPermission {
                        name: f.name,
                        kind: f.kind,
                        path: path.display().to_string(),
                        group,
                    }),
                );
            }
            return;
        }
        Err(_) => return,
    };
    let id = DeviceId(s.next.fetch_add(1, Ordering::SeqCst));
    open.insert(path.clone(), id);
    drop(open);
    emit(
        s,
        Event::Added(DeviceInfo {
            id,
            name: f.name.clone(),
            kind: f.kind,
            via,
            path: path.display().to_string(),
        }),
    );
    let shared = Arc::clone(s);
    let name = format!("plat-dev-{}", id.0 % 100);
    let spawned = specular_thread::builder(name).spawn(move || {
        let r = match (f.kind, via) {
            (Kind::SpaceMouse, _) => {
                let mut rd = PuckReader::default();
                pump::<{ evdev::RECORD }>(&shared, file, id, |rec, out| {
                    rd.event(InputEvent::parse(rec), id, out)
                })
            }
            (_, Via::Joydev) => {
                let mut rd = JsReader::new(&f.caps);
                pump::<{ joydev::RECORD }>(&shared, file, id, |rec, out| {
                    rd.event(JsEvent::parse(rec), id, out)
                })
            }
            _ => {
                let mut rd = PadReader::new(&f.caps);
                pump::<{ evdev::RECORD }>(&shared, file, id, |rec, out| {
                    rd.event(InputEvent::parse(rec), id, out)
                })
            }
        };
        finish(&shared, &path, id, &f.name, r);
    });
    if spawned.is_err()
        && let Ok(mut open) = s.open.lock()
    {
        open.retain(|_, v| *v != id);
    }
}

/// The device is gone (or reading failed): say so and forget it.
fn finish(s: &Shared, path: &Path, id: DeviceId, name: &str, r: std::io::Result<()>) {
    if let Ok(mut open) = s.open.lock() {
        open.remove(path);
    }
    if s.stop.load(Ordering::SeqCst) {
        return;
    }
    emit(s, Event::Removed(id));
    if let Err(e) = r
        // ENODEV (19): unplugged.
        && e.raw_os_error() != Some(19)
        && e.kind() != ErrorKind::UnexpectedEof
    {
        emit(
            s,
            Event::Problem(Problem::ReadFailed {
                name: name.to_owned(),
                error: e.to_string(),
            }),
        );
    }
}

/// Reads whole records until the stream ends, the device goes or the readers stop.
fn pump<const N: usize>(
    s: &Shared,
    mut r: impl Read,
    _id: DeviceId,
    mut f: impl FnMut(&[u8; N], &mut Vec<Event>),
) -> std::io::Result<()> {
    let mut rec = [0u8; N];
    let mut out = Vec::new();
    loop {
        match r.read_exact(&mut rec) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        if s.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        f(&rec, &mut out);
        for e in out.drain(..) {
            emit(s, e);
        }
    }
}

/// Connects to spacenavd if it runs and is not connected yet; true while connected.
fn connect_spnav(s: &Arc<Shared>) -> bool {
    let path = s.roots.spnav.clone();
    let Ok(open) = s.open.lock() else {
        return false;
    };
    if open.contains_key(&path) {
        return true;
    }
    drop(open);
    if !path.exists() {
        return false;
    }
    let Ok(stream) = UnixStream::connect(&path) else {
        return false;
    };
    let Ok(reader) = stream.try_clone() else {
        return false;
    };
    let id = DeviceId(s.next.fetch_add(1, Ordering::SeqCst));
    if let Ok(mut open) = s.open.lock() {
        open.insert(path.clone(), id);
    }
    if let Ok(mut g) = s.spnav.lock() {
        *g = Some(stream);
    }
    emit(
        s,
        Event::Added(DeviceInfo {
            id,
            name: "SpaceMouse".into(),
            kind: Kind::SpaceMouse,
            via: Via::Spacenavd,
            path: path.display().to_string(),
        }),
    );
    let shared = Arc::clone(s);
    let spawned = specular_thread::builder("plat-spnav").spawn(move || {
        let r = pump::<{ spnav::PACKET }>(&shared, reader, id, |p, out| {
            if let Some(e) = Packet::parse(p).and_then(|p| p.event(id)) {
                out.push(e);
            }
        });
        if let Ok(mut g) = shared.spnav.lock() {
            *g = None;
        }
        finish(&shared, &path, id, "SpaceMouse", r);
    });
    spawned.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evdev::{EV_KEY, EV_REL, feed};
    use crate::pad::*;
    use crate::sysfs::tests::{fake_device, scratch};
    use crate::{Button, Devices, Puck, State};
    use specular_input::PadButton;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;

    fn collect() -> (Sink, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        (
            Arc::new(move |e| {
                let _ = tx.lock().unwrap().send(e);
            }),
            rx,
        )
    }

    /// Events until one matches `stop` (or 5 s pass).
    fn until(rx: &mpsc::Receiver<Event>, stop: impl Fn(&Event) -> bool) -> Vec<Event> {
        let mut v = Vec::new();
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(5)) {
            let done = stop(&e);
            v.push(e);
            if done {
                break;
            }
        }
        v
    }

    fn roots(root: &Path) -> Roots {
        Roots {
            class: root.join("sys/class"),
            dev: root.join("dev"),
            spnav: root.join("spnav.sock"),
            groups: root.join("group"),
            watch: false,
        }
    }

    #[test]
    fn fake_pad_and_spacemouse_nodes() {
        let root = scratch("nodes");
        let sys = root.join("sys");
        let dev = root.join("dev");
        std::fs::create_dir_all(&dev).unwrap();
        let pad_keys = [BTN_SOUTH, BTN_EAST, BTN_NORTH, BTN_WEST, BTN_TL, BTN_TR];
        fake_device(
            &sys,
            3,
            "Fake Pad",
            (0x045e, 0x0b13),
            &pad_keys,
            &[0, 1, 2, 3, 4, 5],
            &[],
            &["js0", "event5"],
        );
        fake_device(
            &sys,
            4,
            "Fake SpaceNavigator",
            (0x046d, 0xc626),
            &[0x100, 0x101],
            &[],
            &[0, 1, 2, 3, 4, 5],
            &["event6"],
        );
        // js0: init burst, the left stick right, A pressed.
        let js = |kind, number, value: i16| {
            JsEvent {
                time: 0,
                value,
                kind,
                number,
            }
            .encode()
        };
        let mut bytes = Vec::new();
        for r in [
            js(joydev::JS_EVENT_AXIS | joydev::JS_EVENT_INIT, 0, 0),
            js(joydev::JS_EVENT_AXIS, 0, 32767),
            js(joydev::JS_EVENT_BUTTON, 0, 1),
        ] {
            bytes.extend_from_slice(&r);
        }
        std::fs::write(dev.join("js0"), &bytes).unwrap();
        std::fs::write(dev.join("event5"), b"").unwrap();
        std::fs::write(
            dev.join("event6"),
            feed(&[
                &[InputEvent::new(EV_REL, 0, 350)],
                &[InputEvent::new(EV_KEY, 0x100, 1)],
            ]),
        )
        .unwrap();
        let (sink, rx) = collect();
        let d = Devices::start_at(roots(&root), sink);
        let mut seen = Vec::new();
        let mut removed = 0;
        while removed < 2 {
            let Ok(e) = rx.recv_timeout(Duration::from_secs(5)) else {
                break;
            };
            if matches!(e, Event::Removed(_)) {
                removed += 1;
            }
            seen.push(e);
        }
        drop(d);
        let added: Vec<_> = seen
            .iter()
            .filter_map(|e| match e {
                Event::Added(i) => Some((i.name.as_str(), i.kind, i.via)),
                _ => None,
            })
            .collect();
        assert!(
            added.contains(&("Fake Pad", Kind::Gamepad, Via::Joydev)),
            "{seen:?}"
        );
        assert!(
            added.contains(&("Fake SpaceNavigator", Kind::SpaceMouse, Via::Evdev)),
            "{seen:?}"
        );
        assert!(seen.iter().any(|e| matches!(
            e,
            Event::Button {
                button: Button::Pad(PadButton::A),
                down: true,
                ..
            }
        )));
        assert!(seen.iter().any(|e| matches!(
            e,
            Event::Button {
                button: Button::Puck(1),
                down: true,
                ..
            }
        )));
        assert!(
            seen.iter()
                .any(|e| matches!(e, Event::State(_, State::Pad(p)) if p.left[0] == 1.0))
        );
        assert!(
            seen.iter()
                .any(|e| matches!(e, Event::State(_, State::Puck(p)) if p.t[0] == 1.0))
        );
        assert_eq!(
            removed, 2,
            "a file feed ends like an unplugged device: {seen:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unreadable_node_names_its_group() {
        // Root reads anything: nothing to check then.
        if std::fs::read_to_string("/proc/self/status").is_ok_and(|s| {
            s.lines()
                .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
        }) {
            return;
        }
        let root = scratch("perm");
        let sys = root.join("sys");
        let dev = root.join("dev");
        std::fs::create_dir_all(&dev).unwrap();
        fake_device(
            &sys,
            3,
            "Locked Pad",
            (0x045e, 0x0b13),
            &[BTN_SOUTH],
            &[0, 1],
            &[],
            &["event5"],
        );
        let node = dev.join("event5");
        std::fs::write(&node, b"").unwrap();
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o000)).unwrap();
        let gid = std::fs::metadata(&node).unwrap().gid();
        std::fs::write(
            root.join("group"),
            format!("root:x:0:\nwheel:x:10:\ninput:x:{gid}:alice,bob\n"),
        )
        .unwrap();
        let (sink, rx) = collect();
        let d = Devices::start_at(roots(&root), sink);
        let got = until(&rx, |e| matches!(e, Event::Problem(_)));
        assert_eq!(
            got.last(),
            Some(&Event::Problem(Problem::NoPermission {
                name: "Locked Pad".into(),
                kind: Kind::Gamepad,
                path: node.display().to_string(),
                group: Some("input".into()),
            }))
        );
        // Not reported twice by itself; again after Look again.
        d.inner.rescan();
        let again = until(&rx, |e| matches!(e, Event::Problem(_)));
        assert!(matches!(again.last(), Some(Event::Problem(_))));
        drop(d);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn group_file_lookup() {
        let root = scratch("group");
        let g = root.join("group");
        std::fs::write(&g, "root:x:0:\n# junk\ninput:x:104:\nplugdev:x:46:me\n").unwrap();
        assert_eq!(group_name(&g, 104).as_deref(), Some("input"));
        assert_eq!(group_name(&g, 46).as_deref(), Some("plugdev"));
        assert_eq!(group_name(&g, 999), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fake_spacenavd_socket() {
        let root = scratch("spnav");
        let sock = root.join("spnav.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        // The daemon holds the SpaceMouse: its event node must not be opened too.
        let sys = root.join("sys");
        std::fs::create_dir_all(root.join("dev")).unwrap();
        fake_device(
            &sys,
            4,
            "Fake SpaceNavigator",
            (0x046d, 0xc626),
            &[0x100],
            &[],
            &[0, 1, 2],
            &["event6"],
        );
        #[allow(clippy::disallowed_methods)]
        let server = std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            use std::io::Write;
            for p in [
                Packet::Motion {
                    t: [0, -350, 0],
                    r: [0, 0, 0],
                    period: 16,
                },
                Packet::Button {
                    number: 0,
                    down: true,
                },
                Packet::Button {
                    number: 0,
                    down: false,
                },
                Packet::Motion {
                    t: [0; 3],
                    r: [0; 3],
                    period: 16,
                },
            ] {
                c.write_all(&p.encode()).unwrap();
            }
            // Half a packet, then the daemon quits.
            c.write_all(&[0u8; 7]).unwrap();
        });
        let (sink, rx) = collect();
        let d = Devices::start_at(roots(&root), sink);
        let got = until(&rx, |e| matches!(e, Event::Removed(_)));
        server.join().unwrap();
        drop(d);
        let id = match &got[0] {
            Event::Added(i) => {
                assert_eq!((i.kind, i.via), (Kind::SpaceMouse, Via::Spacenavd));
                i.id
            }
            e => panic!("{e:?} in {got:?}"),
        };
        assert_eq!(
            got[1..],
            [
                Event::State(
                    id,
                    State::Puck(Puck {
                        t: [0.0, -1.0, 0.0],
                        r: [0.0; 3]
                    })
                ),
                Event::Button {
                    id,
                    button: Button::Puck(1),
                    down: true
                },
                Event::Button {
                    id,
                    button: Button::Puck(1),
                    down: false
                },
                Event::State(id, State::Puck(Puck::default())),
                Event::Removed(id),
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
