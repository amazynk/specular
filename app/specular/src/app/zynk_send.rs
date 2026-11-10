//! Handing files to the Zynk desktop app: "Send with Zynk…" gives them to Zynk's picker of
//! contacts and the user's devices, "Share with Zynk…" to its web share form, which makes a
//! web link to them.
//!
//! The work is done by [`crate::zynk`] on a background thread, and what it finds comes back
//! through a channel drained once per event-loop turn (`drain_zynk`). When Zynk has to be
//! started first, which takes a while, a toast says so meanwhile; then the toast goes (Zynk is
//! in front, with the files), or says what failed.
//!
//! Both are always offered. Whether Zynk is installed is looked up when one is chosen (so one
//! installed meanwhile is used at once); without it, a sheet says what Zynk is, how to set it
//! up here, and opens where to get it.
//!
//! On Linux the worker first waits briefly for an activation token from the compositor (or
//! X server): it is what lets Zynk's window come to the front, which Wayland grants only that
//! way.

use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use specular_i18n::t;

use super::{App, items};
use crate::error_text::io_reason;
use crate::zynk::{self, Mode, Unavailable};

/// Accepting the sheet that offers Zynk: where to get it opens.
pub(super) const GET_ITEM: u64 = items::ZYNK_GET;

/// What the background work found.
enum Found {
    /// Zynk can't take the files for the mode: the sheet offering it, or why not.
    Unavailable(Mode, Unavailable),
    /// Zynk has to be started first.
    Opening,
    /// The hand-over is done: the failure to show, or none (Zynk has the files).
    Done(Option<String>),
}

pub(super) struct ZynkState {
    tx: Sender<Found>,
    rx: Receiver<Found>,
    /// The "Opening Zynk…" toast, while one shows.
    toast: Option<u64>,
    /// Linux: the activation token asked for the hand-over under way, by the request's
    /// serial, and the worker waiting for it.
    #[cfg(all(unix, not(target_os = "macos")))]
    token: Option<(
        specular_gfx::winit::event_loop::AsyncRequestSerial,
        Sender<String>,
    )>,
}

/// How long the worker waits for an activation token before handing over without one (the
/// window may then not come forward).
const TOKEN_WAIT: Duration = Duration::from_secs(1);

impl ZynkState {
    pub(super) fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx,
            rx,
            toast: None,
            #[cfg(all(unix, not(target_os = "macos")))]
            token: None,
        }
    }
}

impl App {
    /// "Share with Zynk…": the files go to Zynk's web share form.
    pub(super) fn share_with_zynk(&mut self) {
        self.hand_to_zynk(Mode::Share);
    }

    /// "Send with Zynk…": the files go to Zynk, which asks who gets them.
    pub(super) fn send_with_zynk(&mut self) {
        self.hand_to_zynk(Mode::Send);
    }

    fn hand_to_zynk(&mut self, mode: Mode) {
        // Folders as well as files: Zynk sends and shares a folder whole.
        let files = if self.gallery.active {
            self.gallery.targets()
        } else {
            self.fileops_targets()
        };
        if files.is_empty() {
            return;
        }
        let token = self.request_zynk_token();
        let tx = self.zynk.tx.clone();
        self.loader.background("zynk", move |l| {
            if let Err(why) = zynk::available(mode) {
                let _ = tx.send(Found::Unavailable(mode, why));
                l.waker()();
                return;
            }
            // Those gone since they were listed are left out (looked at here: a slow disk
            // would hold the window).
            let files: Vec<_> = files.into_iter().filter(|p| p.exists()).collect();
            if files.is_empty() {
                return;
            }
            // Only when Zynk has to start (some seconds); a running one is in front at once,
            // and a toast there would only flash.
            if !zynk::running() {
                let _ = tx.send(Found::Opening);
                l.waker()();
            }
            let token = token.and_then(|rx| rx.recv_timeout(TOKEN_WAIT).ok());
            let failed = zynk::hand_over(mode, &files, token.as_deref())
                .err()
                .map(|e| t!("zynk-failed", reason = io_reason(&e)));
            let _ = tx.send(Found::Done(failed));
            l.waker()();
        });
    }

    /// Linux: asks for an activation token for Zynk's window; it comes as a window event
    /// ([`Self::zynk_token`]) to the receiver returned.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn request_zynk_token(&mut self) -> Option<Receiver<String>> {
        use specular_gfx::winit::platform::startup_notify::WindowExtStartupNotify;
        let serial = self.gfx.as_ref()?.window.request_activation_token().ok()?;
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.zynk.token = Some((serial, tx));
        Some(rx)
    }

    /// Elsewhere Zynk's window comes forward without one.
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    fn request_zynk_token(&mut self) -> Option<Receiver<String>> {
        None
    }

    /// Linux: the activation token asked for SERIAL, for the hand-over waiting on it.
    #[cfg(all(unix, not(target_os = "macos")))]
    pub(super) fn zynk_token(
        &mut self,
        serial: specular_gfx::winit::event_loop::AsyncRequestSerial,
        token: specular_gfx::winit::window::ActivationToken,
    ) {
        if let Some((_, tx)) = self.zynk.token.take_if(|(asked, _)| *asked == serial) {
            let _ = tx.send(token.into_raw());
        }
    }

    /// A Zynk worker that panicked: the "Opening Zynk…" toast goes (the warning that the task
    /// failed says the rest).
    pub(super) fn zynk_lost(&mut self, task: &str) {
        if task == "zynk"
            && let Some(id) = self.zynk.toast.take()
        {
            self.chrome.dismiss_toast(id);
        }
    }

    /// The sheet offering Zynk, chosen for MODE without it: what Zynk is, how to set it up
    /// here, and Get Zynk.
    fn offer_zynk(&mut self, mode: Mode) {
        let title = match mode {
            Mode::Send => t!("zynk-get-title-send"),
            Mode::Share => t!("zynk-get-title-share"),
        };
        let detail = format!("{}\n\n{}", t!("zynk-get-about"), get_zynk_here());
        self.chrome
            .confirm_with_detail(title, detail, t!("zynk-get"), GET_ITEM);
    }

    /// The sheet offering Zynk accepted: where to get it opens in the browser (or the store).
    pub(super) fn zynk_get(&mut self) {
        super::ai::open_url(zynk::DOWNLOAD_URL.to_owned());
    }

    /// What the Zynk workers sent back: that Zynk can't take the files (the sheet offering
    /// it, or a toast saying why); that it is starting (the toast says so, up for a while);
    /// that it has the files (it is in front now, with them), or the toast says why not.
    pub(super) fn drain_zynk(&mut self) {
        while let Ok(found) = self.zynk.rx.try_recv() {
            let now = self.now();
            match found {
                Found::Unavailable(mode, Unavailable::NotFound) => self.offer_zynk(mode),
                #[cfg(windows)]
                Found::Unavailable(_, Unavailable::OldWindows) => {
                    self.chrome.toast(t!("zynk-send-old-windows"), None, now);
                }
                Found::Opening => {
                    if self.zynk.toast.is_none() {
                        self.zynk.toast = Some(self.chrome.warning(t!("zynk-opening"), now));
                    }
                }
                Found::Done(failed) => match (self.zynk.toast.take(), failed) {
                    (Some(id), Some(text)) => {
                        self.chrome.update_toast(id, text, now);
                    }
                    (None, Some(text)) => {
                        self.chrome.toast(text, None, now);
                    }
                    (Some(id), None) => self.chrome.dismiss_toast(id),
                    (None, None) => {}
                },
            }
            self.redraw = true;
        }
    }
}

/// How to get Zynk on this system, and set it up.
fn get_zynk_here() -> String {
    #[cfg(target_os = "macos")]
    return t!("zynk-get-macos");
    #[cfg(windows)]
    return t!("zynk-get-windows");
    #[cfg(not(any(target_os = "macos", windows)))]
    return t!("zynk-get-linux");
}
