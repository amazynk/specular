//! "Set as wallpaper": the Wallpaper portal (`org.freedesktop.portal.Wallpaper`, works in
//! sandboxes and on every desktop whose portal backend implements it: GNOME, KDE, …), else
//! the desktop's own mechanism:
//!
//! | Desktop | Fallback |
//! |---|---|
//! | GNOME, Unity, Budgie | `gsettings set org.gnome.desktop.background picture-uri[-dark]` |
//! | Cinnamon | `gsettings set org.cinnamon.desktop.background picture-uri` |
//! | MATE | `gsettings set org.mate.background picture-filename` |
//! | KDE Plasma | `plasma-apply-wallpaperimage` |
//! | Xfce | `xfconf-query` on every `…/last-image` property |
//! | sway | `swaymsg output * bg FILE fill` (until sway restarts; the sway config is not edited) |

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use specular_log::{SETTINGS, debug, info, warn};
use zbus::zvariant::{Fd, OwnedValue, Value};

use super::dbus::session_bus_exists;
use super::run_tool;
use crate::integration::{Env, Error, Result, Unavailable, WallpaperBackend};

const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_IFACE: &str = "org.freedesktop.portal.Wallpaper";
/// The portal may show a confirmation; give the user time.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

/// Sets `image` as the background (blocks; call off the UI thread).
pub fn set(env: &Env, image: &Path) -> Result<WallpaperBackend> {
    let image = image
        .canonicalize()
        .map_err(crate::integration::io_err(image))?;
    let mut last_err = None;
    if session_bus_exists() {
        match portal(&image) {
            Ok(()) => {
                info!(target: SETTINGS, "wallpaper: set {} through the portal", image.display());
                return Ok(WallpaperBackend::Portal);
            }
            Err(e) => {
                debug!(target: SETTINGS, "wallpaper: portal: {e}");
                last_err = Some(e);
            }
        }
    }
    if env.sandboxed {
        return Err(last_err.unwrap_or(Error::Unavailable(Unavailable::Sandboxed)));
    }
    for backend in fallbacks(env) {
        match fallback(env, backend, &image) {
            Ok(true) => {
                info!(target: SETTINGS, "wallpaper: set {} with {}", image.display(), backend.name());
                return Ok(backend);
            }
            Ok(false) => {}
            Err(e) => {
                warn!(target: SETTINGS, "wallpaper: {}: {e}", backend.name());
                last_err = Some(e);
            }
        }
    }
    Err(
        last_err.unwrap_or(Error::Unavailable(Unavailable::MissingTool(
            "xdg-desktop-portal",
        ))),
    )
}

/// A cheap guess whether [`set`] can work.
pub fn available(env: &Env) -> bool {
    let portal = env.data_dirs.iter().any(|d| {
        d.join("dbus-1/services")
            .join(format!("{PORTAL_BUS}.service"))
            .exists()
    });
    (portal && session_bus_exists())
        || fallbacks(env)
            .into_iter()
            .any(|b| env.find(program(b)).is_some())
}

/// The desktop-specific mechanisms to try, most likely first.
fn fallbacks(env: &Env) -> Vec<WallpaperBackend> {
    use WallpaperBackend::*;
    let mut v = Vec::new();
    for (names, b) in [
        (&["GNOME", "Unity", "Budgie", "Pantheon"][..], Gnome),
        (&["X-Cinnamon", "Cinnamon"][..], Cinnamon),
        (&["MATE"][..], Mate),
        (&["KDE"][..], Kde),
        (&["XFCE"][..], Xfce),
        (&["sway"][..], Sway),
    ] {
        if names.iter().any(|n| env.is_desktop(n)) {
            v.push(b);
        }
    }
    if std::env::var_os("SWAYSOCK").is_some() && !v.contains(&Sway) {
        v.push(Sway);
    }
    v
}

fn program(b: WallpaperBackend) -> &'static str {
    match b {
        WallpaperBackend::Gnome | WallpaperBackend::Cinnamon | WallpaperBackend::Mate => {
            "gsettings"
        }
        WallpaperBackend::Kde => "plasma-apply-wallpaperimage",
        WallpaperBackend::Xfce => "xfconf-query",
        WallpaperBackend::Sway => "swaymsg",
        WallpaperBackend::Portal => "xdg-desktop-portal",
        WallpaperBackend::Windows(_) | WallpaperBackend::MacOs => "",
    }
}

fn fallback(env: &Env, b: WallpaperBackend, image: &Path) -> Result<bool> {
    let uri = crate::launch::file_uri(image);
    let path = image.as_os_str();
    let os = |s: &'static str| OsStr::new(s);
    match b {
        WallpaperBackend::Gnome => {
            let schema = os("org.gnome.desktop.background");
            let ok = run_tool(
                env,
                "gsettings",
                &[os("set"), schema, os("picture-uri"), uri.as_ref()],
            )?;
            if ok {
                // GNOME 42+ has a separate dark-style picture; older versions lack the key.
                let _ = run_tool(
                    env,
                    "gsettings",
                    &[os("set"), schema, os("picture-uri-dark"), uri.as_ref()],
                );
            }
            Ok(ok)
        }
        WallpaperBackend::Cinnamon => run_tool(
            env,
            "gsettings",
            &[
                os("set"),
                os("org.cinnamon.desktop.background"),
                os("picture-uri"),
                uri.as_ref(),
            ],
        ),
        WallpaperBackend::Mate => run_tool(
            env,
            "gsettings",
            &[
                os("set"),
                os("org.mate.background"),
                os("picture-filename"),
                path,
            ],
        ),
        WallpaperBackend::Kde => run_tool(env, "plasma-apply-wallpaperimage", &[path]),
        WallpaperBackend::Xfce => {
            let Some(xq) = env.find("xfconf-query").filter(|_| env.run_tools) else {
                return Ok(false);
            };
            let out = Command::new(&xq)
                .args(["-c", "xfce4-desktop", "-l"])
                .output()
                .map_err(|e| Error::Tool("xfconf-query".into(), e.to_string()))?;
            let props: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| l.ends_with("/last-image"))
                .map(str::to_owned)
                .collect();
            if props.is_empty() {
                return Ok(false);
            }
            for p in &props {
                run_tool(
                    env,
                    "xfconf-query",
                    &[
                        os("-c"),
                        os("xfce4-desktop"),
                        os("-p"),
                        p.as_ref(),
                        os("-s"),
                        path,
                    ],
                )?;
            }
            Ok(true)
        }
        WallpaperBackend::Sway => {
            // swaymsg parses its arguments as one command; quote the path for it.
            let quoted = format!(
                "\"{}\"",
                image
                    .to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
            );
            run_tool(
                env,
                "swaymsg",
                &[os("output"), os("*"), os("bg"), quoted.as_ref(), os("fill")],
            )
        }
        WallpaperBackend::Portal | WallpaperBackend::Windows(_) | WallpaperBackend::MacOs => {
            Ok(false)
        }
    }
}

/// `org.freedesktop.portal.Wallpaper.SetWallpaperFile`, waiting for the user's answer.
fn portal(image: &Path) -> Result<()> {
    let bus = |e: zbus::Error| Error::Bus(e.to_string());
    let conn = zbus::blocking::Connection::session().map_err(bus)?;
    // No property cache: where the interface is missing it would only log a warning.
    let proxy = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy>::new(&conn)
        .destination(PORTAL_BUS)
        .and_then(|b| b.path(PORTAL_PATH))
        .and_then(|b| b.interface(PORTAL_IFACE))
        .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
        .and_then(|b| b.build())
        .map_err(bus)?;
    // Without a backend implementing it, the interface is missing.
    proxy
        .get_property::<u32>("version")
        .map_err(|e| Error::Bus(format!("no Wallpaper portal: {e}")))?;

    let file = std::fs::File::open(image).map_err(crate::integration::io_err(image))?;
    let token = format!("specular{}", std::process::id());
    let sender = conn
        .unique_name()
        .map(|n| n.trim_start_matches(':').replace('.', "_"))
        .ok_or_else(|| Error::Bus("no unique name".into()))?;
    let request_path = format!("{PORTAL_PATH}/request/{sender}/{token}");
    // Subscribe before calling, so the answer cannot be missed.
    let request = zbus::blocking::Proxy::new(
        &conn,
        PORTAL_BUS,
        request_path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .map_err(bus)?;
    let mut responses = request.receive_signal("Response").map_err(bus)?;

    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("show-preview", Value::from(false));
    options.insert("set-on", Value::from("background"));
    proxy
        .call_method("SetWallpaperFile", &("", Fd::from(&file), options))
        .map_err(bus)?;

    let (tx, rx) = std::sync::mpsc::channel();
    specular_thread::builder("plat-wallpaper")
        .spawn(move || {
            if let Some(msg) = responses.next() {
                let _ = tx.send(
                    msg.body()
                        .deserialize::<(u32, HashMap<String, OwnedValue>)>()
                        .map(|(code, _)| code),
                );
            }
        })
        .map_err(|e| Error::Bus(e.to_string()))?;
    match rx.recv_timeout(RESPONSE_TIMEOUT) {
        Ok(Ok(0)) => Ok(()),
        Ok(Ok(1)) => Err(Error::Bus("cancelled".into())),
        Ok(Ok(code)) => Err(Error::Bus(format!("the portal answered {code}"))),
        Ok(Err(e)) => Err(Error::Bus(e.to_string())),
        Err(_) => Err(Error::Bus("no answer from the portal".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_env;
    use super::*;

    #[test]
    fn fallbacks_follow_the_desktop() {
        let mut env = test_env::env("wallpaper");
        env.desktops = vec!["ubuntu".into(), "GNOME".into()];
        assert_eq!(fallbacks(&env)[0], WallpaperBackend::Gnome);
        env.desktops = vec!["KDE".into()];
        assert_eq!(fallbacks(&env)[0], WallpaperBackend::Kde);
        env.desktops = vec!["X-Cinnamon".into()];
        assert_eq!(fallbacks(&env)[0], WallpaperBackend::Cinnamon);
        // Tools off (tests): nothing runs and nothing is claimed.
        let img = env.exe.clone();
        assert!(!fallback(&env, WallpaperBackend::Kde, &img).unwrap());
        test_env::cleanup(&env);
    }
}
