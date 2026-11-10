//! Finding renderers on a worker: one SSDP search, each answer's description fetched, the
//! renderers handed back as they come. Only while the app asks (its menu is open): dropping
//! the [`Finder`] stops it.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use specular_core::CancelToken;

use crate::device::{self, Device};
use crate::{Error, ssdp};

/// Wakes the app when something arrives.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// How long one description may take to fetch.
const DESCRIPTION_TIMEOUT: Duration = Duration::from_secs(3);

/// What the search reports.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    Device(Box<Device>),
    /// The search is over (with why it failed, if it did).
    Done(Option<Error>),
}

/// A search running.
pub struct Finder {
    cancel: CancelToken,
    rx: mpsc::Receiver<Found>,
}

impl Finder {
    /// The next report, if one came.
    pub fn try_recv(&self) -> Option<Found> {
        self.rx.try_recv().ok()
    }
}

impl Drop for Finder {
    fn drop(&mut self) {
        // The worker sees it within a tenth of a second (or after the description it is
        // fetching) and ends on its own: the app does not wait.
        self.cancel.cancel();
    }
}

/// Searches `target` (the SSDP group, or a test's responder) for `wait`.
pub fn find(target: SocketAddr, wait: Duration, wake: Wake) -> Finder {
    let cancel = CancelToken::new();
    let (tx, rx) = mpsc::channel();
    let c = cancel.clone();
    specular_thread::spawn("net-cast-find", move || {
        let mut udns: Vec<String> = Vec::new();
        let r = ssdp::search(target, wait, &c, |a| {
            match device::fetch(&a.location, DESCRIPTION_TIMEOUT) {
                Ok(d) if !udns.contains(&d.udn) => {
                    udns.push(d.udn.clone());
                    if tx.send(Found::Device(Box::new(d))).is_ok() {
                        wake();
                    }
                }
                Ok(_) => {}
                Err(e) => specular_log::debug!(
                    target: specular_log::IO,
                    "cast: {} ({}): {e}",
                    a.location,
                    a.from
                ),
            }
        });
        let _ = tx.send(Found::Done(r.err()));
        wake();
    });
    Finder { cancel, rx }
}
