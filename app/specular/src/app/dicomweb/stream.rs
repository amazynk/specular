//! A series streamed from a DICOMweb server into series mode, or retrieved from a DIMSE
//! server by C-GET or C-MOVE ([`specular_dicomweb::retrieve`]) and then opened the same way.
//!
//! [`SeriesLoad`] fetches the metadata, then the middle slice alone, then the rest nearest
//! the slice on screen, into the disk cache. Series mode opens at the first slice that
//! arrives (normally the middle one) with every slice's cache path already listed; slices
//! that arrive later are re-read if they were tried before they came. A toast counts the
//! slices and has a Stop button; leaving series mode stops the load.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use specular_dicom::series::Series;
use specular_dicomweb::retrieve::{self, DimseLoad, Failure, How};
use specular_dicomweb::servers::Retrieve;
use specular_dicomweb::{DicomWeb, LoadEvent, LoadOptions, SeriesLoad, Server};
use specular_i18n::t;
use specular_remote::DiskCache;

use super::super::App;
use super::{error_text, send};

/// The undo key of the loading toast's Stop button.
pub(in crate::app) const STOP_KEY: u64 = 0x0D1C_0E3B_0000_0001;

/// The disk cache's budget for DICOMweb data.
const CACHE_BUDGET: u64 = 2 << 30;
/// C-GET and C-MOVE: the server may take a while between sub-operations.
const RETRIEVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// What fetches the series.
enum Load {
    Web(SeriesLoad),
    Dimse(DimseLoad),
}

impl Load {
    fn cancel(&self) {
        match self {
            Load::Web(l) => l.cancel(),
            Load::Dimse(l) => l.cancel(),
        }
    }

    fn set_focus(&self, k: usize) {
        if let Load::Web(l) = self {
            l.set_focus(k);
        }
    }
}

/// Why a DIMSE retrieval opened nothing, in words.
pub(super) fn retrieve_text(f: &Failure, s: &Server) -> String {
    use specular_dimse::Error as E;
    let host = s.host.trim().to_owned();
    let ae = s.calling_ae().to_owned();
    match f {
        Failure::Dimse(E::NoContext(_) | E::NothingAccepted) if s.retrieve == Retrieve::Move => {
            t!("pacs-retrieve-no-move", host = host)
        }
        Failure::Dimse(E::NoContext(_) | E::NothingAccepted) => {
            t!("pacs-retrieve-no-get", host = host)
        }
        Failure::Dimse(e) => send::dimse_fatal(e, s),
        Failure::Status(0xA801) => t!(
            "pacs-retrieve-unknown-destination",
            host = host,
            ae = ae,
            port = s.receive_port.to_string()
        ),
        Failure::Status(0xA700..=0xA7FF) => t!("pacs-retrieve-resources", host = host),
        Failure::Status(c) => t!(
            "pacs-retrieve-status",
            host = host,
            code = format!("{c:04X}")
        ),
        Failure::NoMatches => t!("pacs-retrieve-no-matches", host = host),
        Failure::NothingArrived {
            roles_refused: true,
            ..
        } => t!("pacs-retrieve-roles", host = host),
        Failure::NothingArrived { moved: true, .. } => {
            t!("pacs-retrieve-move-nothing", host = host, ae = ae)
        }
        Failure::NothingArrived { failed, .. } => t!(
            "pacs-retrieve-nothing",
            host = host,
            count = i64::from(*failed)
        ),
    }
}

pub(in crate::app) struct Stream {
    load: Load,
    rx: Receiver<LoadEvent>,
    server: String,
    /// The server itself (for DIMSE failures, which name its AE titles and port).
    from: Server,
    label: String,
    sets: Vec<Series<PathBuf>>,
    /// Slice index (over all sets) of each cache path.
    index: HashMap<PathBuf, usize>,
    total: usize,
    done: usize,
    failed: usize,
    /// Series mode shows the stream.
    entered: bool,
    /// The load ended (the stream stays to keep its cache entries while viewed).
    finished: bool,
    toast: Option<u64>,
    focus: usize,
}

impl Stream {
    fn progress_text(&self) -> String {
        t!(
            "dicomweb-loading-progress",
            series = self.label.clone(),
            done = self.done as i64,
            total = self.total as i64
        )
    }
}

impl App {
    /// The disk cache, opened on first use.
    fn dicomweb_cache(&mut self) -> Result<Arc<DiskCache>, String> {
        let dir = self.dicomweb.cache_dir.clone().unwrap_or_else(|| {
            specular_settings::Locations::detect()
                .cache_dir
                .join("dicomweb")
        });
        DiskCache::open(dir, CACHE_BUDGET).map_err(|e| crate::error_text::reason(&e))
    }

    /// Starts streaming `series` of `study`; false (with a warning) when it cannot start.
    pub(super) fn dicomweb_stream_start(
        &mut self,
        web: DicomWeb,
        server: &Server,
        study: &str,
        series: &str,
        label: String,
    ) -> bool {
        let prefix = server.cache_prefix();
        self.dicomweb_load_start(server, label, |cache, sink| {
            Load::Web(SeriesLoad::start(
                web,
                cache,
                &prefix,
                study,
                series,
                LoadOptions::default(),
                sink,
            ))
        })
    }

    /// Retrieves `series` of `study` from a DIMSE server by C-GET or C-MOVE, then opens it;
    /// `Err` says why it cannot start, `Ok(false)`: it could not (a warning was shown).
    pub(super) fn dicomweb_retrieve_start(
        &mut self,
        server: &Server,
        study: &str,
        series: &str,
        label: String,
    ) -> Result<bool, String> {
        let options = send::dimse_options(server, RETRIEVE_TIMEOUT)
            .map_err(|e| send::dimse_fatal(&e, server))?;
        let how = match server.retrieve {
            Retrieve::Get => How::Get,
            Retrieve::Move => {
                let address = server
                    .receive_address
                    .trim()
                    .parse()
                    .map_err(|_| t!("pacs-problem-receive-address"))?;
                if server.receive_port == 0 {
                    return Err(t!("pacs-problem-receive-port"));
                }
                How::Move(specular_dimse::listen::Listen {
                    address,
                    port: server.receive_port,
                    ae_title: server.calling_ae().to_owned(),
                })
            }
        };
        let source = retrieve::Source {
            peer: send::peer_of(server),
            options,
            how,
            label: label.clone(),
            registry: None,
        };
        let prefix = server.cache_prefix();
        Ok(self.dicomweb_load_start(server, label, |cache, sink| {
            Load::Dimse(DimseLoad::start(
                source, cache, &prefix, study, series, sink,
            ))
        }))
    }

    fn dicomweb_load_start(
        &mut self,
        server: &Server,
        label: String,
        make: impl FnOnce(Arc<DiskCache>, specular_dicomweb::progressive::Sink) -> Load,
    ) -> bool {
        let now = self.now();
        let cache = match self.dicomweb_cache() {
            Ok(c) => c,
            Err(e) => {
                self.chrome
                    .warning(t!("dicomweb-cache-failed", reason = e), now);
                return false;
            }
        };
        self.dicomweb_stream_stop(false);
        let (tx, rx) = crossbeam_channel::unbounded();
        let wake = self.loader.waker();
        let sink = Arc::new(move |e: LoadEvent| {
            let _ = tx.send(e);
            wake();
        });
        let load = make(cache, sink);
        let toast = self.chrome.toast_action(
            t!("dicomweb-loading", series = label.clone()),
            t!("dicomweb-stop"),
            STOP_KEY,
            now,
        );
        self.chrome
            .announce_status(t!("dicomweb-loading", series = label.clone()));
        self.dicomweb.stream = Some(Stream {
            load,
            rx,
            server: server.label(),
            from: server.clone(),
            label,
            sets: Vec::new(),
            index: HashMap::new(),
            total: 0,
            done: 0,
            failed: 0,
            entered: false,
            finished: false,
            toast: Some(toast),
            focus: usize::MAX,
        });
        true
    }

    /// Stops the load (`announce`: say so).
    fn dicomweb_stream_stop(&mut self, announce: bool) {
        if let Some(s) = self.dicomweb.stream.as_mut()
            && !s.finished
        {
            s.load.cancel();
            if announce {
                self.chrome.dismiss_undo(STOP_KEY);
                self.chrome.announce_status(t!("dicomweb-stopped"));
            }
        }
        if !announce {
            self.dicomweb.stream = None;
        }
    }

    /// The loading toast's Stop button; false when `u` is not its key.
    pub(in crate::app) fn dicomweb_stop_key(&mut self, u: u64) -> bool {
        if u != STOP_KEY {
            return false;
        }
        self.dicomweb_stream_stop(true);
        true
    }

    /// Events from the load, and the slice on screen to it.
    pub(super) fn dicomweb_stream_tick(&mut self) {
        let Some(s) = self.dicomweb.stream.as_ref() else {
            return;
        };
        let events: Vec<LoadEvent> = s.rx.try_iter().collect();
        for e in events {
            self.dicomweb_stream_event(e);
        }
        let Some(s) = self.dicomweb.stream.as_ref() else {
            return;
        };
        if !s.entered {
            return;
        }
        let here = self
            .current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| s.index.get(p).copied());
        match here {
            // Series mode was left, or shows other files.
            None if !self.series_active() || self.current.is_some() => {
                if !s.finished {
                    s.load.cancel();
                    self.chrome.dismiss_undo(STOP_KEY);
                }
                self.dicomweb.stream = None;
            }
            None => {}
            Some(k) if k != s.focus => {
                s.load.set_focus(k);
                if let Some(s) = self.dicomweb.stream.as_mut() {
                    s.focus = k;
                }
            }
            Some(_) => {}
        }
    }

    fn dicomweb_stream_event(&mut self, e: LoadEvent) {
        let now = self.now();
        let Some(s) = self.dicomweb.stream.as_mut() else {
            return;
        };
        match e {
            LoadEvent::Metadata { series, offline } => {
                s.index.clear();
                let mut k = 0;
                for set in &series {
                    for it in &set.items {
                        s.index.insert(it.key.clone(), k);
                        k += 1;
                    }
                }
                s.total = k;
                s.sets = series;
                if offline {
                    self.chrome.warning(
                        t!("dicomweb-offline-cached", server = s.server.clone()),
                        now,
                    );
                }
            }
            LoadEvent::Instance { index, path } => {
                if !s.entered {
                    s.entered = true;
                    let sets = std::mem::take(&mut s.sets);
                    let set = sets
                        .iter()
                        .position(|set| set.items.iter().any(|it| it.key == path))
                        .unwrap_or(0);
                    s.focus = index;
                    let toast = s.toast;
                    let text = s.progress_text();
                    self.series_enter_streamed(sets, set, &path);
                    // Series mode's own toast came after ours: put the progress back on top.
                    if toast.is_some() {
                        self.chrome.dismiss_undo(STOP_KEY);
                        let id = self
                            .chrome
                            .toast_action(text, t!("dicomweb-stop"), STOP_KEY, now);
                        if let Some(s) = self.dicomweb.stream.as_mut() {
                            s.toast = Some(id);
                        }
                    }
                } else {
                    self.reload_path(&path);
                }
            }
            LoadEvent::Failed { index, error } => {
                s.failed += 1;
                specular_log::warn!(target: specular_log::UI, "dicomweb slice {index}: {error}");
            }
            LoadEvent::Progress { done, total } => {
                s.done = done;
                s.total = total;
                let text = s.progress_text();
                if let Some(id) = s.toast {
                    // Keep the Stop button: a toast that timed out comes back with one.
                    let same = self.chrome.update_toast(id, text.clone(), now);
                    if same != id {
                        self.chrome.dismiss_toast(same);
                        let id = self
                            .chrome
                            .toast_action(text, t!("dicomweb-stop"), STOP_KEY, now);
                        if let Some(s) = self.dicomweb.stream.as_mut() {
                            s.toast = Some(id);
                        }
                    }
                }
            }
            LoadEvent::Done {
                failed,
                offline,
                auth,
                cancelled,
            } => {
                s.finished = true;
                let entered = s.entered;
                let (server, label, total) = (s.server.clone(), s.label.clone(), s.total);
                self.chrome.dismiss_undo(STOP_KEY);
                let text = if cancelled {
                    t!("dicomweb-stopped")
                } else if auth {
                    t!("dicomweb-error-auth")
                } else if offline {
                    t!("dicomweb-error-offline", server = server)
                } else if failed > 0 {
                    t!(
                        "dicomweb-loaded-failed",
                        failed = failed as i64,
                        total = total as i64
                    )
                } else {
                    t!("dicomweb-loaded", series = label, total = total as i64)
                };
                if failed > 0 || auth || offline {
                    self.chrome.warning(text.clone(), now);
                } else {
                    self.chrome.toast(text.clone(), None, now);
                }
                self.chrome.announce_status(text);
                if !entered {
                    self.dicomweb.stream = None;
                }
            }
            LoadEvent::Error(e) => {
                let text = match &e {
                    specular_dicomweb::Error::Retrieve(f) => retrieve_text(f, &s.from),
                    e => error_text(e, &s.server),
                };
                self.chrome.dismiss_undo(STOP_KEY);
                self.chrome.warning(text.clone(), now);
                self.chrome.announce_status(text);
                self.dicomweb.stream = None;
            }
        }
        self.redraw = true;
    }
}
