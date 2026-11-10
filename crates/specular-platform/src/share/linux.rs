//! Linux: `org.freedesktop.portal.OpenURI.OpenFile` with `ask` (the portal's application
//! chooser, the desktop's "Open with" that also works in a sandbox), and `xdg-email`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use specular_log::{UI, debug};
use zbus::zvariant::{Fd, OwnedValue, Value};

use crate::integration::linux::dbus::session_bus_exists;

const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_IFACE: &str = "org.freedesktop.portal.OpenURI";
/// The chooser waits for the user.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);

fn bus_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("D-Bus: {e}"))
}

/// Whether a portal is likely there (its D-Bus service is installed and a bus is running).
pub(super) fn portal_available() -> bool {
    if !session_bus_exists() {
        return false;
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("XDG_DATA_DIRS")
        .map(|d| std::env::split_paths(&d).collect())
        .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]);
    if let Some(h) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(h.into());
    }
    dirs.iter().any(|d| {
        d.join("dbus-1/services")
            .join(format!("{PORTAL_BUS}.service"))
            .exists()
    })
}

/// Asks the portal to open `file`, letting the user choose the application.
pub(super) fn open_file(file: &Path) -> io::Result<()> {
    let conn = zbus::blocking::Connection::session().map_err(bus_err)?;
    let proxy = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy>::new(&conn)
        .destination(PORTAL_BUS)
        .and_then(|b| b.path(PORTAL_PATH))
        .and_then(|b| b.interface(PORTAL_IFACE))
        .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
        .and_then(|b| b.build())
        .map_err(bus_err)?;
    let f = std::fs::File::open(file)?;
    let token = format!("specularshare{}", std::process::id());
    let sender = conn
        .unique_name()
        .map(|n| n.trim_start_matches(':').replace('.', "_"))
        .ok_or_else(|| bus_err("no unique name"))?;
    let request_path = format!("{PORTAL_PATH}/request/{sender}/{token}");
    // Subscribe before calling, so the answer cannot be missed.
    let request = zbus::blocking::Proxy::new(
        &conn,
        PORTAL_BUS,
        request_path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .map_err(bus_err)?;
    let mut responses = request.receive_signal("Response").map_err(bus_err)?;
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("ask", Value::from(true));
    proxy
        .call_method("OpenFile", &("", Fd::from(&f), options))
        .map_err(bus_err)?;
    let (tx, rx) = std::sync::mpsc::channel();
    specular_thread::builder("plat-share")
        .spawn(move || {
            if let Some(msg) = responses.next() {
                let _ = tx.send(
                    msg.body()
                        .deserialize::<(u32, HashMap<String, OwnedValue>)>()
                        .map(|(code, _)| code),
                );
            }
        })
        .map_err(io::Error::other)?;
    match rx.recv_timeout(RESPONSE_TIMEOUT) {
        // 1: the user closed the chooser; nothing to report.
        Ok(Ok(0 | 1)) => Ok(()),
        Ok(Ok(code)) => Err(bus_err(format!("the portal answered {code}"))),
        Ok(Err(e)) => Err(bus_err(e)),
        Err(_) => {
            debug!(target: UI, "share: no answer from the portal");
            Ok(())
        }
    }
}

/// `xdg-email --attach FILE…`: the user's mail program with a new message.
pub(super) fn email(files: &[PathBuf]) -> io::Result<()> {
    let program = crate::launch::find_program("xdg-email")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "xdg-email"))?;
    let refs: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    let status = std::process::Command::new(program)
        .args(super::email_args(&refs))
        .stdin(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("xdg-email: {status}")))
    }
}
