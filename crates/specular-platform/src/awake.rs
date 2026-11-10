//! Keeping the screen awake while a movie plays (not while paused, nor for still images).
//!
//! - Linux and the BSDs: `org.freedesktop.ScreenSaver.Inhibit` on the session bus (KDE,
//!   GNOME, Xfce, most screen lockers), else the desktop portal's
//!   `org.freedesktop.portal.Inhibit` (idle), released with `UnInhibit` or by closing the
//!   portal's request. Both tie the hold to our bus connection: it ends with the process,
//!   crash included.
//! - Windows: `Windows.System.Display.DisplayRequest` (safe WinRT), released with
//!   `RequestRelease`; Windows ends it with the process.
//! - macOS: an `NSProcessInfo` activity with idle display sleep disabled, ended when the
//!   hold is dropped (the system ends it with the process).
//!
//! The calls run on a worker thread: a slow or absent bus never holds up the UI.

use std::sync::mpsc;

/// A hold on the screen, turned on and off as playback starts and stops.
pub struct Awake {
    tx: Option<mpsc::Sender<Msg>>,
    on: bool,
    /// Where the worker reports what it did (tests).
    #[cfg(test)]
    seen: Option<mpsc::Receiver<bool>>,
}

enum Msg {
    Set(bool),
}

impl Default for Awake {
    fn default() -> Self {
        Awake::new()
    }
}

impl std::fmt::Debug for Awake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Awake").field("on", &self.on).finish()
    }
}

/// A platform's way to hold the screen awake. Made on the worker thread and kept there
/// (WinRT and AppKit objects stay on the thread that made them).
trait Backend {
    /// Starts a hold; `false` when nothing could take it.
    fn hold(&mut self, reason: &str) -> bool;
    fn release(&mut self);
}

impl Awake {
    /// A hold that does nothing until [`Awake::set`] turns it on. With `SPECULAR_NO_INHIBIT`
    /// set (tests, headless runs) it never reaches the system.
    pub fn new() -> Awake {
        let backend: Option<Make> =
            if std::env::var_os("SPECULAR_NO_INHIBIT").is_some() || cfg!(test) {
                None
            } else {
                system()
            };
        Awake::with(backend)
    }

    fn with(backend: Option<Make>) -> Awake {
        #[cfg(test)]
        let (seen_tx, seen) = mpsc::channel();
        let tx = backend.and_then(|make| {
            let (tx, rx) = mpsc::channel::<Msg>();
            specular_thread::builder("platform-awake")
                .spawn(move || {
                    let mut b = make();
                    let mut held = false;
                    while let Ok(Msg::Set(on)) = rx.recv() {
                        if on && !held {
                            held = b.hold("Playing a video");
                        } else if !on && held {
                            b.release();
                            held = false;
                        }
                        #[cfg(test)]
                        let _ = seen_tx.send(held);
                    }
                    // The viewer closed: let go.
                    if held {
                        b.release();
                    }
                })
                .ok()
                .map(|_| tx)
        });
        Awake {
            tx,
            on: false,
            #[cfg(test)]
            seen: Some(seen),
        }
    }

    /// Holds the screen awake (`true`) or lets it sleep again.
    pub fn set(&mut self, on: bool) {
        if on == self.on {
            return;
        }
        self.on = on;
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Set(on));
        }
        specular_log::debug!(target: specular_log::STARTUP, "screen awake: {on}");
    }

    pub fn is_on(&self) -> bool {
        self.on
    }
}

/// Makes a backend on the worker thread.
type Make = Box<dyn FnOnce() -> Box<dyn Backend> + Send>;

/// The system's backend, when this platform has one.
fn system() -> Option<Make> {
    #[cfg(all(unix, not(target_os = "macos")))]
    return Some(Box::new(|| {
        Box::new(linux::Bus::session()) as Box<dyn Backend>
    }));
    #[cfg(windows)]
    return Some(Box::new(|| {
        Box::new(windows::Display::default()) as Box<dyn Backend>
    }));
    #[cfg(target_os = "macos")]
    return Some(Box::new(|| Box::new(Mac(None)) as Box<dyn Backend>));
    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "macos")]
struct Mac(Option<crate::Activity>);

#[cfg(target_os = "macos")]
impl Backend for Mac {
    fn hold(&mut self, reason: &str) -> bool {
        self.0 = Some(crate::Activity {
            _inner: crate::macos::power::begin_awake(reason),
        });
        true
    }
    fn release(&mut self) {
        self.0 = None;
    }
}

#[cfg(windows)]
mod windows {
    use windows::System::Display::DisplayRequest;

    #[derive(Default)]
    pub(super) struct Display(Option<DisplayRequest>);

    impl super::Backend for Display {
        fn hold(&mut self, _reason: &str) -> bool {
            let r = DisplayRequest::new().and_then(|r| r.RequestActive().map(|()| r));
            match r {
                Ok(r) => {
                    self.0 = Some(r);
                    true
                }
                Err(e) => {
                    specular_log::debug!(target: specular_log::STARTUP, "display request: {e}");
                    false
                }
            }
        }
        fn release(&mut self) {
            if let Some(r) = self.0.take() {
                let _ = r.RequestRelease();
            }
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod linux {
    use std::collections::HashMap;

    use zbus::zvariant::{OwnedObjectPath, Value};

    const SS_NAME: &str = "org.freedesktop.ScreenSaver";
    const SS_PATHS: [&str; 2] = ["/org/freedesktop/ScreenSaver", "/ScreenSaver"];
    const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
    const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
    /// The portal's "idle" flag.
    const INHIBIT_IDLE: u32 = 8;

    enum Hold {
        ScreenSaver { path: &'static str, cookie: u32 },
        Portal(OwnedObjectPath),
    }

    /// The session bus (or, in tests, a private one at `address`).
    pub(crate) struct Bus {
        address: Option<String>,
        conn: Option<zbus::blocking::Connection>,
        hold: Option<Hold>,
    }

    impl Bus {
        pub(crate) fn session() -> Bus {
            Bus {
                address: None,
                conn: None,
                hold: None,
            }
        }

        #[cfg(test)]
        pub(crate) fn at(address: &str) -> Bus {
            Bus {
                address: Some(address.to_owned()),
                conn: None,
                hold: None,
            }
        }

        fn conn(&mut self) -> Option<&zbus::blocking::Connection> {
            if self.conn.is_none() {
                let c = match &self.address {
                    Some(a) => zbus::blocking::connection::Builder::address(a.as_str())
                        .and_then(|b| b.build()),
                    None => zbus::blocking::Connection::session(),
                };
                match c {
                    Ok(c) => self.conn = Some(c),
                    Err(e) => {
                        specular_log::debug!(target: specular_log::STARTUP, "screen awake: no session bus: {e}");
                        return None;
                    }
                }
            }
            self.conn.as_ref()
        }

        fn screensaver(&mut self, reason: &str) -> Option<Hold> {
            let conn = self.conn()?;
            for path in SS_PATHS {
                let r = conn.call_method(
                    Some(SS_NAME),
                    path,
                    Some(SS_NAME),
                    "Inhibit",
                    &("Specular", reason),
                );
                if let Ok(m) = r
                    && let Ok(cookie) = m.body().deserialize::<u32>()
                {
                    return Some(Hold::ScreenSaver { path, cookie });
                }
            }
            None
        }

        fn portal(&mut self, reason: &str) -> Option<Hold> {
            let conn = self.conn()?;
            let mut opts: HashMap<&str, Value> = HashMap::new();
            opts.insert("reason", Value::from(reason));
            let m = conn
                .call_method(
                    Some(PORTAL_NAME),
                    PORTAL_PATH,
                    Some("org.freedesktop.portal.Inhibit"),
                    "Inhibit",
                    &("", INHIBIT_IDLE, opts),
                )
                .ok()?;
            m.body()
                .deserialize::<OwnedObjectPath>()
                .ok()
                .map(Hold::Portal)
        }
    }

    impl super::Backend for Bus {
        fn hold(&mut self, reason: &str) -> bool {
            self.hold = self.screensaver(reason).or_else(|| self.portal(reason));
            if self.hold.is_none() {
                specular_log::debug!(target: specular_log::STARTUP, "screen awake: no screen saver or portal answered");
            }
            self.hold.is_some()
        }

        fn release(&mut self) {
            let Some(h) = self.hold.take() else { return };
            let Some(conn) = self.conn() else { return };
            let _ = match h {
                Hold::ScreenSaver { path, cookie } => {
                    conn.call_method(Some(SS_NAME), path, Some(SS_NAME), "UnInhibit", &(cookie,))
                }
                Hold::Portal(p) => conn.call_method(
                    Some(PORTAL_NAME),
                    p.as_str(),
                    Some("org.freedesktop.portal.Request"),
                    "Close",
                    &(),
                ),
            };
        }
    }
}

#[cfg(test)]
mod tests;
