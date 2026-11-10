//! Display profiles on Linux/BSD: Wayland colour management, colord, X11 `_ICC_PROFILE`.

use std::path::PathBuf;

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle};

use super::{DisplayProfile, DisplayQuery, read_icc};

pub(super) fn profile(q: &DisplayQuery) -> DisplayProfile {
    if wayland_managed() {
        return DisplayProfile::Managed;
    }
    if let Some(name) = &q.monitor
        && let Some(path) = colord(name)
        && let Some(icc) = read_icc(&path)
    {
        return DisplayProfile::Icc(icc);
    }
    match x11_atom(q.index.unwrap_or(0)) {
        Some(icc) => DisplayProfile::Icc(icc),
        None => DisplayProfile::Unknown,
    }
}

struct Globals;

impl Dispatch<WlRegistry, GlobalListContents> for Globals {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// Whether a Wayland compositor with the colour-management protocol is running (its own
/// short-lived connection; only the global list is read).
fn wayland_managed() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return false;
    }
    let Ok(conn) = Connection::connect_to_env() else {
        return false;
    };
    let Ok((globals, _queue)) = registry_queue_init::<Globals>(&conn) else {
        return false;
    };
    globals
        .contents()
        .with_list(|l| l.iter().any(|g| g.interface == "wp_color_manager_v1"))
}

/// The file of the default profile colord has for the RandR output `name`.
fn colord(name: &str) -> Option<PathBuf> {
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::OwnedObjectPath;

    const DEST: &str = "org.freedesktop.ColorManager";
    let run = || -> zbus::Result<Option<PathBuf>> {
        let conn = Connection::system()?;
        let manager = Proxy::new(&conn, DEST, "/org/freedesktop/ColorManager", DEST)?;
        let device: OwnedObjectPath =
            match manager.call("FindDeviceByProperty", &("XRANDR_name", name)) {
                Ok(d) => d,
                Err(zbus::Error::MethodError(..)) => return Ok(None), // no such device
                Err(e) => return Err(e),
            };
        let device = Proxy::new(&conn, DEST, device, "org.freedesktop.ColorManager.Device")?;
        let profiles: Vec<OwnedObjectPath> = device.get_property("Profiles")?;
        let Some(first) = profiles.into_iter().next() else {
            return Ok(None);
        };
        let profile = Proxy::new(&conn, DEST, first, "org.freedesktop.ColorManager.Profile")?;
        let file: String = profile.get_property("Filename")?;
        Ok((!file.is_empty()).then(|| PathBuf::from(file)))
    };
    match run() {
        Ok(p) => p,
        Err(e) => {
            specular_log::debug!("colord: {e}");
            None
        }
    }
}

/// The `_ICC_PROFILE[_<index>]` property of the root window.
fn x11_atom(index: usize) -> Option<Vec<u8>> {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
    use x11rb::rust_connection::RustConnection;

    std::env::var_os("DISPLAY")?;
    let (c, screen) = RustConnection::connect(None).ok()?;
    let root = c.setup().roots.get(screen)?.root;
    let name = match index {
        0 => "_ICC_PROFILE".to_owned(),
        n => format!("_ICC_PROFILE_{n}"),
    };
    let atom = c
        .intern_atom(true, name.as_bytes())
        .ok()?
        .reply()
        .ok()?
        .atom;
    if atom == 0 {
        return None;
    }
    let reply = c
        .get_property(false, root, atom, AtomEnum::ANY, 0, 16 << 20)
        .ok()?
        .reply()
        .ok()?;
    let v = reply.value;
    (v.len() >= 132 && &v[36..40] == b"acsp").then_some(v)
}
