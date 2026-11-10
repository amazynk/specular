//! Remote control: a per-user local socket every viewer window listens on (DESIGN.md §11,
//! "Scripting"), the line-delimited JSON protocol it speaks ([`protocol`]), and the
//! `specular --remote` client ([`client`]).
//!
//! Security: the socket lives in a folder only its user can enter (`$XDG_RUNTIME_DIR/specular`,
//! mode 0700, or `specular/run` in the cache folder, created 0700), is itself 0600, and the
//! server drops any connection whose peer runs as another user (`SO_PEERCRED` /
//! `getpeereid`). Windows: an `AF_UNIX` socket under `%LOCALAPPDATA%`, whose ACL admits only
//! the user. Requests are bounded (1 MiB per line, 10 000 files, 8 clients, 16 blobs per
//! client, blobs reserved from the memory budget), parsed without panicking, and answered
//! within seconds; nothing in the protocol runs programs or touches files beyond opening
//! images, except `open` with `"window":"new"`, which starts Specular, and `array-show`, which
//! reads (and removes) a bare-named regular file of the private `shared/` folder and writes
//! the arrays it shows into the private `arrays/` folder ([`arrays`]).
//!
//! Version 3 adds events ([`events`]): a connection that subscribes gets the window's events
//! as lines of their own, from a bounded queue that never holds the window up, and has no idle
//! limit (4 such connections besides the 8). Windows has no peer check, so a key file next to
//! the socket stands in: clients send its key in `hello`. `integration.ipc = false` turns the
//! socket off. Hooks (`app/hooks.rs`) are the window's, not the socket's: they run only
//! programs settings.toml names, each after the user allowed it.
//!
//! The protocol's published spec, versions 1 to 3: `docs/ipc.md`.
//!
//! Which way a start hands over to a running viewer:
//!
//! 1. `specular --remote …` uses the socket only (never D-Bus), and fails with status 3 when
//!    no viewer listens.
//! 2. `specular FILE…` (no other options) goes over D-Bus as before, when the single-instance
//!    integration option and setting are on. The socket is not used for this on Linux, so a
//!    viewer without that option keeps opening a window per start. Windows has no D-Bus: there
//!    it goes over the socket ([`forward_files`]) when the single-instance setting is on, with a
//!    named mutex per copy deciding which of several processes started at once opens the
//!    window (`integration_cli`, `specular_platform::instance`).
//! 3. `specular specular://…`: links naming plain local files are file arguments (rule 2);
//!    any other link goes to a running viewer over the socket when the single-instance setting
//!    is on (the confirmation is asked there), else to a new window that asks itself.

pub mod arrays;
pub mod client;
pub mod events;
pub mod protocol;
pub mod server;
#[cfg(test)]
mod tests_python;
#[cfg(test)]
mod tests_v2;
#[cfg(test)]
mod tests_v3;

use std::path::PathBuf;

/// Hands `url` to a running viewer over the socket. True when one took it (it shows the link,
/// asks about it or says why it cannot).
pub fn forward_url(url: &str) -> bool {
    let Some(path) = client::socket_path(None) else {
        return false;
    };
    let Ok(mut c) = client::Client::connect(&path) else {
        return false;
    };
    match c.call(&protocol::Request::OpenUrl {
        url: url.to_owned(),
    }) {
        Ok(_) => true,
        // The viewer refused the link and said so in its window.
        Err(e) => e.code == "refused" || e.code == "bad-arguments",
    }
}

/// Hands `files` (none: just raise the window) to the viewer listening on `socket`, and lets
/// it come to the front. True when it took them; false when none listens or none of the files
/// exist (this process then shows them, or why not, in a window of its own).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn forward_files(socket: &std::path::Path, files: &[PathBuf]) -> bool {
    let Ok(mut c) = client::Client::connect(socket) else {
        return false;
    };
    // Windows lets only the process the user just started come to the front; this one passes
    // that on to the viewer before asking it to raise its window.
    if let Ok(hello) = c.call(&protocol::Request::Hello)
        && let Some(pid) = hello["pid"].as_u64().and_then(|p| u32::try_from(p).ok())
    {
        specular_platform::instance::allow_foreground(pid);
    }
    let req = if files.is_empty() {
        protocol::Request::Activate
    } else {
        protocol::Request::Open {
            files: files.to_vec(),
            window: protocol::Window::Current,
            view: None,
            compare: None,
        }
    };
    c.call(&req).is_ok()
}

/// Hands `files` (2–4) to a running viewer to compare in `mode`. True when one took them;
/// false when none listens (this process then opens a window of its own).
pub fn forward_compare(files: &[PathBuf], mode: &str) -> bool {
    let Some(socket) = client::socket_path(None) else {
        return false;
    };
    let Ok(mut c) = client::Client::connect(&socket) else {
        return false;
    };
    if let Ok(hello) = c.call(&protocol::Request::Hello)
        && let Some(pid) = hello["pid"].as_u64().and_then(|p| u32::try_from(p).ok())
    {
        specular_platform::instance::allow_foreground(pid);
    }
    c.call(&protocol::Request::Open {
        files: files.to_vec(),
        window: protocol::Window::Current,
        view: None,
        compare: Some(mode.to_owned()),
    })
    .is_ok_and(|v| v["compared"].as_bool() == Some(true))
}

/// The socket folder for a viewer that starts listening.
pub fn socket_dir() -> std::io::Result<PathBuf> {
    specular_platform::ipc::socket_dir(&client::fallback_dir())
}
