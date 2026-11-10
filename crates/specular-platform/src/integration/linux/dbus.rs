//! Single instance through D-Bus activation (`org.freedesktop.Application`).
//!
//! The option installs `~/.local/share/dbus-1/services/app.specular.Specular.service` (the bus
//! starts `specular --gapplication-service` on demand) and sets `DBusActivatable=true` in our
//! `.desktop` file, so file managers and launchers hand files to the running Specular through
//! `Open` instead of starting another process.
//!
//! The running viewer owns the name `app.specular.Specular` ([`serve`]) while the option is on
//! and the `general.single_instance` setting allows it; `specular FILE` from a terminal asks
//! the owner to open the file ([`forward`]) and exits. Without the option the viewer stays
//! multi-instance, as before.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use specular_log::{SETTINGS, debug, info, warn};
use zbus::zvariant::{OwnedValue, Value};

use super::{APP_ID, MARKER, Record, install, is_ours, local_desktop_file, shell_quote};
use super::{sync_desktop_file, sync_desktop_file_after_removal, uninstall};
use crate::integration::{Env, Error, Labels, OptionId, Result, State};

/// Object path of `org.freedesktop.Application` (the id with `.` → `/`).
pub const OBJECT_PATH: &str = "/app/specular/Specular";
/// Argument the bus starts us with.
pub const SERVICE_ARG: &str = "--gapplication-service";

/// Whether there is a session bus to talk to.
pub fn session_bus_exists() -> bool {
    std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some_and(|a| !a.is_empty())
        || std::env::var_os("XDG_RUNTIME_DIR").is_some_and(|d| Path::new(&d).join("bus").exists())
}

fn service_file(env: &Env, name: &str) -> PathBuf {
    env.data_home
        .join("dbus-1/services")
        .join(format!("{name}.service"))
}

/// A D-Bus service file starting `exe args` for `name`.
pub(crate) fn service_entry(env: &Env, name: &str, args: &str, why: &str) -> String {
    format!(
        "# {MARKER}: installed by Specular ({why}); removed with it.\n\
         [D-BUS Service]\n\
         Name={name}\n\
         Exec={} {args}\n",
        shell_quote(&env.exe)
    )
}

/// Whether the single-instance option is on (the viewer checks this at start).
pub fn enabled(env: &Env) -> bool {
    is_ours(&service_file(env, APP_ID))
}

pub(super) fn apply(env: &Env, labels: &Labels) -> Result<()> {
    let key = OptionId::DbusActivation.key();
    let mut rec = Record::load(env, key).unwrap_or_default();
    install(
        &mut rec,
        &service_file(env, APP_ID),
        &service_entry(
            env,
            APP_ID,
            SERVICE_ARG,
            "Settings > Integration > Single instance",
        ),
        0o644,
    )?;
    rec.save(env, key)?;
    sync_desktop_file(env, labels)?;
    reload_bus(env);
    Ok(())
}

pub(super) fn remove(env: &Env) -> Result<()> {
    let key = OptionId::DbusActivation.key();
    let mut rec = Record::load(env, key).unwrap_or_default();
    rec.add_file(&service_file(env, APP_ID));
    uninstall(&rec);
    Record::delete(env, key);
    sync_desktop_file_after_removal(env)?;
    reload_bus(env);
    Ok(())
}

pub(super) fn status(env: &Env) -> (State, Vec<String>) {
    let service = enabled(env);
    let desktop = std::fs::read_to_string(local_desktop_file(env))
        .is_ok_and(|t| t.contains(MARKER) && t.lines().any(|l| l.trim() == "DBusActivatable=true"));
    let state = match (service, desktop) {
        (true, true) => State::On,
        (false, false) => State::Off,
        _ => State::Partial,
    };
    (state, Vec::new())
}

/// Asks the session bus to rescan service files (dbus-daemon watches the folders itself;
/// dbus-broker needs this). Best effort.
pub(crate) fn reload_bus(env: &Env) {
    if !env.run_tools || !session_bus_exists() {
        return;
    }
    let r = (|| -> zbus::Result<()> {
        let conn = zbus::blocking::Connection::session()?;
        let bus = zbus::blocking::fdo::DBusProxy::new(&conn)?;
        bus.reload_config()?;
        Ok(())
    })();
    if let Err(e) = r {
        debug!(target: SETTINGS, "integration: D-Bus ReloadConfig: {e}");
    }
}

// ----- The viewer side ------------------------------------------------------------------

/// What another process (a launcher, a file manager, a second `specular`) asks of the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Bring the window forward.
    Activate { token: Option<String> },
    /// Open these files (one: show it in its folder; several: browse exactly those).
    Open {
        files: Vec<PathBuf>,
        token: Option<String>,
    },
    /// A launcher-menu action of the `.desktop` file ([`super::DesktopAction`]), with an
    /// optional folder (`specular --slideshow DIR` forwarded from a terminal) or, for the
    /// Library, a query (`specular --search QUERY`, the desktop search's "more results").
    Action {
        action: super::DesktopAction,
        folder: Option<PathBuf>,
        query: Option<String>,
        token: Option<String>,
    },
}

type Handler = Arc<dyn Fn(Request) + Send + Sync>;

struct AppIface {
    handler: Handler,
}

/// The startup-notification token of a request (`activation-token` on Wayland,
/// `desktop-startup-id` on X11).
fn token(platform_data: &HashMap<String, OwnedValue>) -> Option<String> {
    ["activation-token", "desktop-startup-id"]
        .iter()
        .find_map(|k| match platform_data.get(*k).map(|v| &**v) {
            Some(Value::Str(s)) if !s.is_empty() => Some(s.to_string()),
            _ => None,
        })
}

/// URIs or paths as files to open (non-`file:` URIs are skipped).
pub fn uris_to_paths(uris: &[String]) -> Vec<PathBuf> {
    uris.iter()
        .filter_map(|u| {
            if u.starts_with("file:") {
                crate::launch::path_from_uri(u)
            } else if u.starts_with('/') {
                Some(PathBuf::from(u))
            } else {
                None
            }
        })
        .collect()
}

/// The one string parameter of an action (launchers send no parameter at all).
fn action_text(parameter: &[OwnedValue]) -> Option<String> {
    let [v] = parameter else {
        return None;
    };
    match &**v {
        Value::Str(s) => Some(s.to_string()),
        Value::Value(inner) => match &**inner {
            Value::Str(s) => Some(s.to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// The folder parameter of an action: one absolute path or `file:` URI string.
fn action_folder(parameter: &[OwnedValue]) -> Option<PathBuf> {
    uris_to_paths(&[action_text(parameter)?]).pop()
}

#[zbus::interface(name = "org.freedesktop.Application")]
impl AppIface {
    fn activate(&self, platform_data: HashMap<String, OwnedValue>) {
        debug!(target: SETTINGS, "D-Bus Activate");
        (self.handler)(Request::Activate {
            token: token(&platform_data),
        });
    }

    fn open(&self, uris: Vec<String>, platform_data: HashMap<String, OwnedValue>) {
        let files = uris_to_paths(&uris);
        info!(target: SETTINGS, "D-Bus Open: {} file(s)", files.len());
        (self.handler)(Request::Open {
            files,
            token: token(&platform_data),
        });
    }

    fn activate_action(
        &self,
        action_name: String,
        parameter: Vec<OwnedValue>,
        platform_data: HashMap<String, OwnedValue>,
    ) {
        debug!(target: SETTINGS, "D-Bus ActivateAction {action_name}");
        let token = token(&platform_data);
        let request = match super::DesktopAction::from_id(&action_name) {
            // The Library's parameter is a query, any other's a folder.
            Some(action @ super::DesktopAction::Library) => Request::Action {
                action,
                folder: None,
                query: action_text(&parameter).filter(|q| !q.trim().is_empty()),
                token,
            },
            Some(action) => Request::Action {
                action,
                folder: action_folder(&parameter),
                query: None,
                token,
            },
            // Unknown actions (an older or newer desktop file): at least come forward.
            None => Request::Activate { token },
        };
        (self.handler)(request);
    }
}

/// The viewer's end: owns `app.specular.Specular` while alive.
pub struct Service {
    _conn: zbus::blocking::Connection,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dbus::Service")
    }
}

/// Exports `org.freedesktop.Application` and takes the name. `Ok(None)`: another process owns
/// it already (this one stays a separate instance). `handler` runs on zbus's thread.
pub fn serve(handler: impl Fn(Request) + Send + Sync + 'static) -> Result<Option<Service>> {
    use zbus::fdo::{RequestNameFlags, RequestNameReply};
    let bus = |e: zbus::Error| Error::Bus(e.to_string());
    let conn = zbus::blocking::connection::Builder::session()
        .map_err(bus)?
        .serve_at(
            OBJECT_PATH,
            AppIface {
                handler: Arc::new(handler),
            },
        )
        .map_err(bus)?
        .build()
        .map_err(bus)?;
    match conn
        .request_name_with_flags(APP_ID, RequestNameFlags::DoNotQueue.into())
        .map_err(bus)?
    {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {
            info!(target: SETTINGS, "single instance: serving {APP_ID} on the session bus");
            Ok(Some(Service { _conn: conn }))
        }
        r => {
            debug!(target: SETTINGS, "single instance: {APP_ID} is taken ({r:?})");
            Ok(None)
        }
    }
}

/// Hands `files` (empty: just activate) to a running viewer. `Ok(false)`: none is running
/// (nothing is started: the caller goes on as the first instance).
pub fn forward(files: &[PathBuf]) -> Result<bool> {
    let call = if files.is_empty() {
        Call::Activate
    } else {
        Call::Open(files)
    };
    forward_call(call)
}

/// Asks a running viewer to run a launcher-menu action (`specular --library` from a terminal
/// with the single-instance option on), with a folder, or the Library with a query
/// (`specular --search QUERY`). `Ok(false)`: none is running.
pub fn forward_action(
    action: super::DesktopAction,
    folder: Option<&Path>,
    query: Option<&str>,
) -> Result<bool> {
    forward_call(Call::Action(action, folder, query))
}

enum Call<'a> {
    Activate,
    Open(&'a [PathBuf]),
    Action(super::DesktopAction, Option<&'a Path>, Option<&'a str>),
}

fn forward_call(call: Call<'_>) -> Result<bool> {
    let bus = |e: zbus::Error| Error::Bus(e.to_string());
    if !session_bus_exists() {
        return Ok(false);
    }
    let conn = zbus::blocking::connection::Builder::session()
        .map_err(bus)?
        .method_timeout(Duration::from_secs(5))
        .build()
        .map_err(bus)?;
    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn).map_err(bus)?;
    let name = zbus::names::BusName::try_from(APP_ID).map_err(|e| Error::Bus(e.to_string()))?;
    if !dbus
        .name_has_owner(name)
        .map_err(|e| Error::Bus(e.to_string()))?
    {
        return Ok(false);
    }
    let proxy =
        zbus::blocking::Proxy::new(&conn, APP_ID, OBJECT_PATH, "org.freedesktop.Application")
            .map_err(bus)?;
    let mut data: HashMap<&str, Value> = HashMap::new();
    for (var, key) in [
        ("XDG_ACTIVATION_TOKEN", "activation-token"),
        ("DESKTOP_STARTUP_ID", "desktop-startup-id"),
    ] {
        if let Ok(v) = std::env::var(var)
            && !v.is_empty()
        {
            data.insert(key, Value::from(v));
        }
    }
    let r = match call {
        Call::Activate => proxy.call_method("Activate", &(data,)),
        Call::Open(files) => {
            let uris: Vec<String> = files.iter().map(|p| crate::launch::file_uri(p)).collect();
            proxy.call_method("Open", &(uris, data))
        }
        Call::Action(action, folder, query) => {
            let param: Vec<Value> = match (folder, query) {
                (_, Some(q)) => vec![Value::from(q.to_owned())],
                (Some(f), None) => vec![Value::from(crate::launch::file_uri(f))],
                (None, None) => Vec::new(),
            };
            proxy.call_method("ActivateAction", &(action.id(), param, data))
        }
    };
    match r {
        Ok(_) => Ok(true),
        Err(e) => {
            warn!(target: SETTINGS, "single instance: the running viewer did not answer: {e}");
            Err(bus(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_env;
    use super::*;

    #[test]
    fn option_files() {
        let env = test_env::env("dbus");
        assert_eq!(status(&env).0, State::Off);
        apply(&env, &Labels::default()).unwrap();
        let svc = std::fs::read_to_string(service_file(&env, APP_ID)).unwrap();
        assert!(svc.contains("Name=app.specular.Specular\n"));
        assert!(svc.contains("specular' --gapplication-service\n"), "{svc}");
        let desktop = std::fs::read_to_string(local_desktop_file(&env)).unwrap();
        assert!(desktop.contains("DBusActivatable=true\n"));
        assert_eq!(status(&env).0, State::On);
        assert!(enabled(&env));
        remove(&env).unwrap();
        assert!(!service_file(&env, APP_ID).exists());
        assert!(!local_desktop_file(&env).exists());
        assert_eq!(status(&env).0, State::Off);
        test_env::cleanup(&env);
    }

    #[test]
    fn desktop_file_is_shared_with_the_default_viewer() {
        let env = test_env::env("dbus-shared");
        let l = Labels::default();
        crate::integration::linux::apply(&env, OptionId::DefaultViewer, &l).unwrap();
        let d = local_desktop_file(&env);
        assert!(
            std::fs::read_to_string(&d)
                .unwrap()
                .contains("DBusActivatable=false")
        );
        apply(&env, &l).unwrap();
        assert!(
            std::fs::read_to_string(&d)
                .unwrap()
                .contains("DBusActivatable=true")
        );
        remove(&env).unwrap();
        // Still needed by the default viewer, without D-Bus activation.
        assert!(
            std::fs::read_to_string(&d)
                .unwrap()
                .contains("DBusActivatable=false")
        );
        crate::integration::linux::remove(&env, OptionId::DefaultViewer).unwrap();
        assert!(!d.exists());
        test_env::cleanup(&env);
    }

    #[test]
    fn uris() {
        let u = vec![
            "file:///tmp/a%20b.jpg".to_owned(),
            "/abs/c.png".to_owned(),
            "https://example.com/x.jpg".to_owned(),
        ];
        assert_eq!(
            uris_to_paths(&u),
            [PathBuf::from("/tmp/a b.jpg"), PathBuf::from("/abs/c.png")]
        );
    }
}
