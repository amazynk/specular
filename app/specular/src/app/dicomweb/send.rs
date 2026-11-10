//! Send to PACS: studies from the Studies browser (a patient's disc, or a folder of DICOM
//! files) sent to a server of the list, with C-STORE (`specular-dimse`) or STOW-RS
//! (`specular_dicomweb::stow`), optionally anonymised first (`specular_dicom::anon`, one
//! session for the whole send, so UIDs and the pseudonym stay consistent).
//!
//! The DICOM servers sheet's Send page: what to send (the series, its study or everything
//! read), Anonymise, and the server; Send asks first, naming the server and the counts;
//! then one line of progress per file with Stop, and a summary that names every file that
//! failed and why. Nothing is sent before the confirmation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use specular_core::net::{self, Failure as NetFailure, Label, What};
use specular_dicom::anon::{
    self, AnonError, Options as AnonOptions, Request as AnonRequest, Session,
};
use specular_dicomweb::{DicomWeb, Server, ServerKind, StoreOutcome};
use specular_dimse::{Error as DimseError, Rejection, Status};
use specular_i18n::t;
use specular_remote::CancelToken;

/// What can be sent from where the browser is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Scope {
    Series,
    Study,
    All,
}

/// One choice of what to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct Choice {
    pub scope: Scope,
    pub files: Vec<PathBuf>,
    pub studies: usize,
}

/// What the Send page offers (at least one choice).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct SendRequest {
    pub choices: Vec<Choice>,
    /// The choice shown first.
    pub at: usize,
}

/// A file that did not go: its name (never patient data: the file's own name) and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct Failed {
    pub name: String,
    pub reason: String,
}

/// What the send job reports.
pub(super) enum Event {
    /// One file done: stored (with a warning when `true`), or why not.
    One(Result<bool, Failed>),
    /// The job ended: why the rest could not go, if it stopped early.
    End {
        fatal: Option<String>,
        cancelled: bool,
    },
}

/// The page's stage.
pub(super) enum Stage {
    Choose,
    /// Send pressed: the question, waiting for Send or Back.
    Confirm,
    Running {
        rx: Receiver<Event>,
        cancel: CancelToken,
    },
    Done,
}

/// The Send page's state.
pub(super) struct Sending {
    pub req: SendRequest,
    pub anonymise: bool,
    pub stage: Stage,
    /// The server sent to (its label and host), fixed when the send starts.
    pub server: String,
    pub host: String,
    /// "C-STORE" or "STOW-RS", for the log.
    pub protocol: &'static str,
    pub done: usize,
    pub warnings: usize,
    pub failed: Vec<Failed>,
    pub fatal: Option<String>,
    pub cancelled: bool,
}

impl Sending {
    pub fn new(req: SendRequest) -> Sending {
        Sending {
            req,
            anonymise: true,
            stage: Stage::Choose,
            server: String::new(),
            host: String::new(),
            protocol: "",
            done: 0,
            warnings: 0,
            failed: Vec::new(),
            fatal: None,
            cancelled: false,
        }
    }

    pub fn choice(&self) -> &Choice {
        &self.req.choices[self.req.at.min(self.req.choices.len() - 1)]
    }

    pub fn total(&self) -> usize {
        self.choice().files.len()
    }

    pub fn running(&self) -> bool {
        matches!(self.stage, Stage::Running { .. })
    }

    pub fn sent(&self) -> usize {
        self.done.saturating_sub(self.failed.len())
    }

    /// Steps the scope.
    pub fn step_scope(&mut self, d: i32) {
        let n = self.req.choices.len() as i32;
        self.req.at = (self.req.at as i32 + d).rem_euclid(n.max(1)) as usize;
    }

    /// Takes what the job reported; whether it ended.
    pub fn poll(&mut self) -> bool {
        let Stage::Running { rx, .. } = &self.stage else {
            return false;
        };
        let mut ended = false;
        while let Ok(e) = rx.try_recv() {
            match e {
                Event::One(Ok(warn)) => {
                    self.done += 1;
                    self.warnings += usize::from(warn);
                }
                Event::One(Err(f)) => {
                    self.done += 1;
                    self.failed.push(f);
                }
                Event::End { fatal, cancelled } => {
                    self.fatal = fatal;
                    self.cancelled = cancelled;
                    ended = true;
                }
            }
        }
        if !ended
            && rx.is_empty()
            && matches!(
                rx.try_recv(),
                Err(crossbeam_channel::TryRecvError::Disconnected)
            )
        {
            // The job went away without saying (it panicked): stopped.
            self.fatal = Some(t!("error-reason-internal"));
            ended = true;
        }
        if ended {
            self.stage = Stage::Done;
        }
        ended
    }
}

/// The text of a scope choice.
pub(super) fn scope_text(c: &Choice) -> String {
    let files = c.files.len() as i64;
    match c.scope {
        Scope::Series => t!("pacs-scope-series", files = files),
        Scope::Study => t!("pacs-scope-study", files = files),
        Scope::All => t!("pacs-scope-all", studies = c.studies as i64, files = files),
    }
}

/// A file's name for the summary.
fn name_of(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Why a C-STORE or STOW-RS status failed, in words.
pub(super) fn status_text(code: u16) -> String {
    match Status::of(code) {
        Status::OutOfResources => t!("pacs-reason-resources"),
        Status::DataSetMismatch => t!("pacs-reason-mismatch"),
        Status::CannotUnderstand => t!("pacs-reason-understand"),
        Status::SopClassNotSupported => t!("pacs-reason-class"),
        Status::NotAuthorised => t!("pacs-reason-not-authorised"),
        _ => t!("pacs-reason-status", code = format!("{code:04X}")),
    }
}

/// Why the association failed, naming the host (and the AE title at fault).
pub(super) fn dimse_fatal(e: &DimseError, s: &Server) -> String {
    let host = s.host.trim().to_owned();
    match e {
        DimseError::NetworkOff => t!("pacs-fatal-network-off"),
        DimseError::UnknownHost => t!("pacs-fatal-unknown-host", host = host),
        DimseError::Connect(_) => t!("pacs-fatal-connect", host = host, port = s.port as i64),
        DimseError::Timeout => t!("pacs-fatal-timeout", host = host),
        DimseError::Cancelled => t!("pacs-fatal-cancelled"),
        DimseError::Rejected { .. } => match e.rejection() {
            Some(Rejection::CalledAe) => {
                t!(
                    "pacs-fatal-called-ae",
                    host = host,
                    ae = s.ae_title.trim().to_owned()
                )
            }
            Some(Rejection::CallingAe) => {
                t!(
                    "pacs-fatal-calling-ae",
                    host = host,
                    ae = s.calling_ae().to_owned()
                )
            }
            Some(Rejection::Busy) => t!("pacs-fatal-busy", host = host),
            _ => t!("pacs-fatal-rejected", host = host),
        },
        DimseError::Aborted { .. } | DimseError::Io(_) => t!("pacs-fatal-aborted", host = host),
        DimseError::NothingAccepted => t!("pacs-fatal-nothing-accepted", host = host),
        DimseError::Status(c) => status_text(*c),
        DimseError::Tls(specular_dimse::TlsError::Certificate(_)) => {
            t!("pacs-fatal-tls-certificate", host = host)
        }
        DimseError::Tls(_) => t!("pacs-fatal-tls", host = host, port = s.port as i64),
        DimseError::Listen(e) => t!(
            "pacs-retrieve-listen",
            address = s.receive_address.trim().to_owned(),
            port = s.receive_port.to_string(),
            reason = crate::error_text::io_reason(e)
        ),
        DimseError::Protocol(_) | DimseError::NoContext(_) | DimseError::Dicom(_) => {
            t!("pacs-fatal-protocol", host = host)
        }
    }
}

fn dimse_reason(e: &DimseError) -> String {
    match e {
        DimseError::NoContext(c) => t!(
            "pacs-reason-not-accepted",
            class = specular_dimse::sop::name(c).to_owned()
        ),
        DimseError::Dicom(specular_dicom::Error::NotDicom) => t!("pacs-reason-not-dicom"),
        DimseError::Dicom(d) => t!(
            "pacs-reason-convert",
            reason = crate::error_text::dicom_reason(d)
        ),
        other => other.to_string(),
    }
}

/// Why a DICOMweb request failed, naming the host.
pub(super) fn web_fatal(e: &specular_dicomweb::Error, host: &str) -> String {
    let host = host.to_owned();
    if e.is_network_off() {
        t!("pacs-fatal-network-off")
    } else if e.is_auth() {
        t!("pacs-fatal-auth", host = host)
    } else if e.is_cancelled() {
        t!("pacs-fatal-cancelled")
    } else if e.is_offline() {
        t!("pacs-fatal-unreachable", host = host)
    } else if let Some(s) = e.status() {
        t!("pacs-fatal-http", host = host, status = s as i64)
    } else {
        t!("pacs-fatal-protocol", host = host)
    }
}

/// The file's bytes as they are sent: read, and anonymised when asked.
fn prepare(
    path: &Path,
    session: Option<&Session>,
    cancel: &CancelToken,
) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| {
        t!(
            "pacs-reason-unreadable",
            reason = crate::error_text::io_reason(&e)
        )
    })?;
    let Some(session) = session else {
        return Ok(bytes);
    };
    let opts = AnonOptions::default();
    match anon::anonymise(
        &bytes,
        &AnonRequest {
            options: &opts,
            session,
            masks: &[],
            allow_burned_in: false,
            cancel: Some(cancel.flag()),
        },
    ) {
        Ok(a) => Ok(a.bytes),
        Err(AnonError::BurnedIn) => Err(t!("pacs-reason-burned-in")),
        Err(AnonError::Document) => Err(t!("pacs-reason-document")),
        Err(e) => Err(t!("pacs-reason-anonymise", reason = e.to_string())),
    }
}

/// What the job needs.
pub(super) struct Job {
    pub server: Server,
    /// DICOMweb: the service with its credentials.
    pub web: Option<DicomWeb>,
    pub files: Vec<PathBuf>,
    pub anonymise: bool,
    pub cancel: CancelToken,
    pub tx: Sender<Event>,
    pub wake: Arc<dyn Fn() + Send + Sync>,
    /// Where activity is reported (tests); the global registry otherwise.
    pub registry: Option<net::Registry>,
}

impl Job {
    fn one(&self, r: Result<bool, Failed>) {
        let _ = self.tx.send(Event::One(r));
        (self.wake)();
    }

    fn end(&self, fatal: Option<String>) {
        let cancelled = self.cancel.is_cancelled() && fatal.is_none();
        let _ = self.tx.send(Event::End { fatal, cancelled });
        (self.wake)();
    }

    /// Runs on a worker thread until every file went, the user stopped it, or the server
    /// stopped it.
    pub fn run(self) {
        let session = self.anonymise.then(Session::new);
        let r = match self.server.kind {
            ServerKind::Dimse => self.run_dimse(session.as_ref()),
            ServerKind::DicomWeb => self.run_stow(session.as_ref()),
        };
        self.end(r.err());
    }

    fn run_dimse(&self, session: Option<&Session>) -> Result<(), String> {
        use specular_dimse::{Association, Options, store};
        let s = &self.server;
        let peer = peer_of(s);
        // What to offer: each file's class and syntax (anonymised native data goes as
        // Explicit VR Little Endian, offered for every class anyway).
        let mut heads = Vec::with_capacity(self.files.len());
        let mut total = 0u64;
        for f in &self.files {
            if self.cancel.is_cancelled() {
                return Ok(());
            }
            if let Ok(Some(h)) = store::read_head(f) {
                heads.push((h.sop_class, h.transfer_syntax));
            }
            total += std::fs::metadata(f).map_or(0, |m| m.len());
        }
        let ctx = store::contexts(heads.iter().map(|(c, t)| (c.as_str(), t.as_str())));
        let reg = self
            .registry
            .clone()
            .unwrap_or_else(|| net::global().clone());
        let mut track = if net::off::is_off() {
            None
        } else {
            Some(reg.begin(Label::new(
                What::DicomSend,
                &format!("dicom://{}", peer.address()),
            )))
        };
        if let Some(t) = track.as_mut() {
            t.head(Some(total), 0);
        }
        let finish = |track: Option<net::Request>, r: Result<(), NetFailure>| {
            if let Some(t) = track {
                t.finish(r);
            }
        };
        let opts = match dimse_options(s, Options::default().timeout) {
            Ok(o) => o,
            Err(e) => {
                finish(track, Err(net_failure_dimse(&e)));
                return Err(dimse_fatal(&e, s));
            }
        };
        let mut a = match Association::request(&peer, &opts, &ctx, &self.cancel) {
            Ok(a) => a,
            Err(e) => {
                finish(track, Err(net_failure_dimse(&e)));
                return if matches!(e, DimseError::Cancelled) {
                    Ok(())
                } else {
                    Err(dimse_fatal(&e, s))
                };
            }
        };
        for f in &self.files {
            if self.cancel.is_cancelled() || track.as_ref().is_some_and(|t| t.is_cancelled()) {
                self.cancel.cancel();
                a.abort();
                finish(track, Err(NetFailure::Cancelled));
                return Ok(());
            }
            let name = name_of(f);
            let bytes = match prepare(f, session, &self.cancel) {
                Ok(b) => b,
                Err(reason) => {
                    self.one(Err(Failed { name, reason }));
                    continue;
                }
            };
            let cancel = self.cancel.clone();
            let mut sent = |n: u64| {
                if let Some(t) = track.as_mut() {
                    t.add(n);
                    if t.is_cancelled() {
                        cancel.cancel();
                    }
                }
            };
            match a.send_file(&bytes, Some(self.cancel.flag()), &mut sent) {
                Ok(r) => {
                    let st = Status::of(r.status);
                    if st.stored() {
                        self.one(Ok(st == Status::Warning));
                    } else {
                        self.one(Err(Failed {
                            name,
                            reason: status_text(r.status),
                        }));
                    }
                }
                Err(e) if !e.is_fatal() => self.one(Err(Failed {
                    name,
                    reason: dimse_reason(&e),
                })),
                Err(e) => {
                    finish(track, Err(net_failure_dimse(&e)));
                    return if matches!(e, DimseError::Cancelled) {
                        Ok(())
                    } else {
                        Err(dimse_fatal(&e, s))
                    };
                }
            }
        }
        let r = a.release();
        finish(track, Ok(()));
        // Everything went; a release that is not answered does not undo that.
        if let Err(e) = r {
            specular_log::debug!(target: specular_log::IO, "dimse release: {e}");
        }
        Ok(())
    }

    /// STOW-RS: the files go in batches of at most [`STOW_BATCH_FILES`] files and
    /// [`STOW_BATCH_BYTES`] bytes per request (a larger file goes alone); each file's result
    /// comes from the server's per-instance answer.
    fn run_stow(&self, session: Option<&Session>) -> Result<(), String> {
        let Some(web) = self.web.as_ref() else {
            return Err(t!("error-reason-internal"));
        };
        let web = web.clone().tracked(What::DicomSend);
        let host = net::host_of(&self.server.base_url);
        let mut batch: Vec<Outgoing> = Vec::new();
        let mut bytes_in = 0usize;
        for f in &self.files {
            if self.cancel.is_cancelled() {
                return Ok(());
            }
            let name = name_of(f);
            let bytes = match prepare(f, session, &self.cancel) {
                Ok(b) => b,
                Err(reason) => {
                    self.one(Err(Failed { name, reason }));
                    continue;
                }
            };
            if !batch.is_empty()
                && (batch.len() >= STOW_BATCH_FILES || bytes_in + bytes.len() > STOW_BATCH_BYTES)
            {
                if self.stow_batch(&web, &batch, &host)? {
                    return Ok(());
                }
                batch.clear();
                bytes_in = 0;
            }
            let uid = specular_dimse::store::head(&bytes).map(|h| h.sop_instance);
            bytes_in += bytes.len();
            batch.push(Outgoing { name, bytes, uid });
        }
        if !batch.is_empty() {
            self.stow_batch(&web, &batch, &host)?;
        }
        Ok(())
    }

    /// One STOW-RS request for `batch`; a 413 splits it in halves and sends each. `Ok(true)`:
    /// cancelled; `Err`: the server or network stopped the send.
    fn stow_batch(&self, web: &DicomWeb, batch: &[Outgoing], host: &str) -> Result<bool, String> {
        if self.cancel.is_cancelled() {
            return Ok(true);
        }
        let parts: Vec<&[u8]> = batch.iter().map(|o| o.bytes.as_slice()).collect();
        let fail_all = |reason: String| {
            for o in batch {
                self.one(Err(Failed {
                    name: o.name.clone(),
                    reason: reason.clone(),
                }));
            }
        };
        match web.store(&parts, None, &self.cancel) {
            Ok(r) => {
                for o in batch {
                    // A file whose UID could not be read: what the status says for all.
                    match o
                        .uid
                        .as_deref()
                        .map_or_else(|| r.outcome(""), |u| r.outcome(u))
                    {
                        StoreOutcome::Stored => self.one(Ok(false)),
                        StoreOutcome::Warning(_) => self.one(Ok(true)),
                        StoreOutcome::Failed(code) => self.one(Err(Failed {
                            name: o.name.clone(),
                            reason: code.map_or_else(|| t!("pacs-reason-refused"), status_text),
                        })),
                    }
                }
                Ok(false)
            }
            Err(e) if e.is_cancelled() => Ok(true),
            Err(e) if e.status() == Some(413) && batch.len() > 1 => {
                let (a, b) = batch.split_at(batch.len() / 2);
                if self.stow_batch(web, a, host)? {
                    return Ok(true);
                }
                self.stow_batch(web, b, host)
            }
            Err(e) if e.status() == Some(413) => {
                fail_all(t!("pacs-reason-too-large"));
                Ok(false)
            }
            Err(e) if matches!(e.status(), Some(s) if s < 500 && s != 401 && s != 403) => {
                fail_all(t!(
                    "pacs-reason-http",
                    status = e.status().unwrap_or(0) as i64
                ));
                Ok(false)
            }
            Err(e) => Err(web_fatal(&e, host)),
        }
    }
}

/// STOW-RS batch bounds: files and bytes per request.
const STOW_BATCH_FILES: usize = 64;
const STOW_BATCH_BYTES: usize = 32 << 20;

/// A file ready to go by STOW-RS.
struct Outgoing {
    name: String,
    bytes: Vec<u8>,
    uid: Option<String>,
}

fn net_failure_dimse(e: &DimseError) -> NetFailure {
    match e {
        DimseError::Cancelled => NetFailure::Cancelled,
        DimseError::Timeout => NetFailure::Timeout,
        DimseError::Connect(_) | DimseError::UnknownHost | DimseError::NetworkOff => {
            NetFailure::Offline
        }
        _ => NetFailure::Other,
    }
}

/// The DIMSE peer for `s`.
pub(super) fn peer_of(s: &Server) -> specular_dimse::Peer {
    specular_dimse::Peer {
        host: s.host.trim().to_owned(),
        port: s.port,
        called_ae: s.ae_title.trim().to_owned(),
        calling_ae: s.calling_ae().to_owned(),
    }
}

/// The association options for `s` with this answer `timeout`: TLS verified against the
/// system's trust store when the server is set to it.
pub(super) fn dimse_options(
    s: &Server,
    timeout: std::time::Duration,
) -> Result<specular_dimse::Options, DimseError> {
    Ok(specular_dimse::Options {
        timeout,
        tls: if s.tls {
            Some(specular_dimse::Tls::system()?)
        } else {
            None
        },
        ..specular_dimse::Options::default()
    })
}

/// The C-ECHO (DIMSE) or a one-study search (DICOMweb) behind Verify.
pub(super) fn verify(
    s: &Server,
    web: Option<&DicomWeb>,
    cancel: &CancelToken,
) -> Result<(), String> {
    match s.kind {
        ServerKind::Dimse => {
            let peer = peer_of(s);
            dimse_options(s, std::time::Duration::from_secs(10))
                .and_then(|opts| specular_dimse::assoc::verify(&peer, &opts, cancel))
                .map_err(|e| dimse_fatal(&e, s))
        }
        ServerKind::DicomWeb => {
            let w = web.ok_or_else(|| t!("dicomweb-secret-needed"))?;
            w.check(cancel)
                .map_err(|e| web_fatal(&e, &net::host_of(&s.base_url)))
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The sheet's Send page and Verify.

use super::{Dialog, Outcome, Page, Row, Tone};
use crate::app::App;

impl App {
    /// "Send to PACS…": the sheet on its Send page for `req` (the first server selected).
    pub(in crate::app) fn dicomweb_send(&mut self, req: SendRequest) {
        if req.choices.is_empty() {
            return;
        }
        let busy = self
            .dicomweb
            .dialog
            .as_ref()
            .is_some_and(|d| d.send.as_ref().is_some_and(Sending::running));
        if busy {
            let now = self.now();
            self.chrome.toast(t!("pacs-busy"), None, now);
            return;
        }
        self.dicomweb_close();
        self.dicomweb.load_servers();
        let mut d = Dialog::new();
        d.page = Page::Send;
        if !self.dicomweb.servers.servers.is_empty() {
            d.server = Some(0);
            d.selected = Some(Row::Server(0));
        }
        d.send = Some(Sending::new(req));
        self.dicomweb.dialog = Some(d);
        self.dicomweb_focus_default();
        self.chrome.announce_status(t!("pacs-title"));
        self.redraw = true;
    }

    pub(super) fn dicomweb_step_kind(&mut self) {
        if let Some(e) = self.dicomweb.dialog.as_mut().and_then(|d| d.edit.as_mut()) {
            e.kind = match e.kind {
                ServerKind::DicomWeb => ServerKind::Dimse,
                ServerKind::Dimse => ServerKind::DicomWeb,
            };
            self.chrome.announce_status(super::kind_text(e.kind));
        }
        self.redraw = true;
    }

    pub(super) fn dicomweb_step_scope(&mut self, d: i32) {
        if let Some(s) = self.dicomweb.dialog.as_mut().and_then(|d| d.send.as_mut()) {
            s.step_scope(d);
            self.chrome.announce_status(scope_text(s.choice()));
        }
        self.redraw = true;
    }

    pub(super) fn dicomweb_toggle_anonymise(&mut self) {
        if let Some(s) = self.dicomweb.dialog.as_mut().and_then(|d| d.send.as_mut()) {
            s.anonymise = !s.anonymise;
            self.chrome.announce_status(if s.anonymise {
                t!("pacs-anonymise-note")
            } else {
                t!("pacs-identified-note")
            });
        }
        self.redraw = true;
    }

    /// Send: the question first (server and counts named).
    pub(super) fn dicomweb_send_ask(&mut self) {
        let Some(i) = self.dicomweb.dialog.as_ref().and_then(|d| d.server) else {
            return;
        };
        let Some(server) = self.dicomweb.servers.servers.get(i).cloned() else {
            return;
        };
        let no_secret =
            server.kind == ServerKind::DicomWeb && self.dicomweb.auth_for(&server).is_none();
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return;
        };
        let Some(s) = d.send.as_mut() else { return };
        if !matches!(s.stage, Stage::Choose) || s.choice().files.is_empty() {
            return;
        }
        // DICOMweb with credentials not known yet: they are given in the server's form.
        if no_secret {
            d.message = Some((t!("dicomweb-secret-needed"), Tone::Warning));
            self.redraw = true;
            return;
        }
        s.stage = Stage::Confirm;
        d.message = None;
        self.dicomweb_focus_default();
        if let Some(v) = self.dicomweb_view()
            && let Some((n, _)) = v.notes.first()
        {
            self.chrome.announce_status(n.clone());
        }
        self.redraw = true;
    }

    pub(super) fn dicomweb_send_back(&mut self) {
        if let Some(s) = self.dicomweb.dialog.as_mut().and_then(|d| d.send.as_mut())
            && matches!(s.stage, Stage::Confirm)
        {
            s.stage = Stage::Choose;
        }
        self.dicomweb_focus_default();
        self.redraw = true;
    }

    pub(super) fn dicomweb_send_stop(&mut self) {
        if let Some(s) = self.dicomweb.dialog.as_ref().and_then(|d| d.send.as_ref())
            && let Stage::Running { cancel, .. } = &s.stage
        {
            cancel.cancel();
            self.chrome.announce_status(t!("pacs-stopping"));
        }
        self.redraw = true;
    }

    /// Confirmed: the send starts on a worker.
    pub(super) fn dicomweb_send_start(&mut self) {
        let Some(i) = self.dicomweb.dialog.as_ref().and_then(|d| d.server) else {
            return;
        };
        let Some(server) = self.dicomweb.servers.servers.get(i).cloned() else {
            return;
        };
        let web = match server.kind {
            ServerKind::DicomWeb => match self.dicomweb.web(i) {
                Ok(w) => Some(w),
                Err(e) => {
                    if let Some(d) = self.dicomweb.dialog.as_mut() {
                        let m = e.unwrap_or_else(|| t!("dicomweb-secret-needed"));
                        d.message = Some((m, Tone::Error));
                    }
                    self.redraw = true;
                    return;
                }
            },
            ServerKind::Dimse => None,
        };
        let wake = self.loader.waker();
        let Some(s) = self.dicomweb.dialog.as_mut().and_then(|d| d.send.as_mut()) else {
            return;
        };
        if !matches!(s.stage, Stage::Confirm) {
            return;
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        let cancel = CancelToken::new();
        s.server = server.label();
        s.protocol = match server.kind {
            ServerKind::Dimse => "C-STORE",
            ServerKind::DicomWeb => "STOW-RS",
        };
        s.host = match server.kind {
            ServerKind::Dimse => format!("{}:{}", server.host.trim(), server.port),
            ServerKind::DicomWeb => net::host_of(&server.base_url),
        };
        s.done = 0;
        s.warnings = 0;
        s.failed.clear();
        s.fatal = None;
        s.cancelled = false;
        s.stage = Stage::Running {
            rx,
            cancel: cancel.clone(),
        };
        let job = Job {
            server,
            web,
            files: s.choice().files.clone(),
            anonymise: s.anonymise,
            cancel,
            tx,
            wake,
            registry: None,
        };
        specular_log::info!(target: specular_log::IO, "send to PACS: {} files to {}{}",
            job.files.len(), s.host, if job.anonymise { ", anonymised" } else { "" });
        self.loader.background("dcm-send", move |_| job.run());
        self.dicomweb_focus_default();
        self.chrome.announce_status(t!("pacs-started"));
        self.redraw = true;
    }

    /// What the send job reported; at its end, the summary is announced.
    pub(super) fn dicomweb_send_tick(&mut self) {
        let Some(s) = self.dicomweb.dialog.as_mut().and_then(|d| d.send.as_mut()) else {
            return;
        };
        let before = s.done;
        let ended = s.poll();
        if s.done != before || ended {
            self.redraw = true;
        }
        if ended {
            specular_log::info!(target: specular_log::IO, "send to PACS ({}): {} of {} sent to {}, {} failed{}", s.protocol,
                s.sent(), s.total(), s.host, s.failed.len(),
                if s.cancelled { ", stopped" } else { "" });
            let what = s.protocol;
            let extra = [
                ("host", serde_json::json!(s.host)),
                ("sent", serde_json::json!(s.sent())),
                ("failed", serde_json::json!(s.failed.len())),
                ("warnings", serde_json::json!(s.warnings)),
                ("total", serde_json::json!(s.total())),
                ("anonymised", serde_json::json!(s.anonymise)),
                (
                    "stopped",
                    serde_json::json!(s.cancelled || s.fatal.is_some()),
                ),
            ];
            self.audit(crate::app::audit::Event::Send, what, &[], &extra);
            self.dicomweb_focus_default();
            if let Some(v) = self.dicomweb_view()
                && let Some((msg, _)) = v.status
            {
                self.chrome.announce_status(msg);
            }
        }
    }

    /// Verify, on the server's form as it stands: C-ECHO for a DIMSE server, a one-study
    /// search for DICOMweb (with the password or token typed, else the one kept), on a
    /// worker. Nothing is saved.
    pub(super) fn dicomweb_verify(&mut self) {
        let Some(form) = self.dicomweb.dialog.as_ref().and_then(Dialog::form_server) else {
            return;
        };
        let server = match form {
            Ok(s) => s,
            Err(msg) => {
                if let Some(d) = self.dicomweb.dialog.as_mut() {
                    d.message = Some((msg, Tone::Error));
                }
                self.redraw = true;
                return;
            }
        };
        let secret = self
            .dicomweb
            .dialog
            .as_ref()
            .and_then(|d| d.edit.as_ref())
            .map(|e| e.secret.text().to_owned())
            .unwrap_or_default();
        let web = match server.kind {
            ServerKind::DicomWeb => {
                let auth = match server.auth {
                    specular_remote::AuthKind::None => Some(specular_remote::Auth::None),
                    k if !secret.is_empty() => Some(if k == specular_remote::AuthKind::Basic {
                        specular_remote::Auth::Basic {
                            user: server.user.clone(),
                            password: specular_remote::Secret::new(secret),
                        }
                    } else {
                        specular_remote::Auth::Bearer(specular_remote::Secret::new(secret))
                    }),
                    _ => self.dicomweb.auth_for(&server),
                };
                let (Some(auth), Ok(client)) = (auth, self.dicomweb.client()) else {
                    if let Some(d) = self.dicomweb.dialog.as_mut() {
                        d.message = Some((t!("dicomweb-secret-needed"), Tone::Warning));
                    }
                    self.redraw = true;
                    return;
                };
                Some(DicomWeb::new(client, &server.base_url, auth))
            }
            ServerKind::Dimse => None,
        };
        let (tx, rx) = crossbeam_channel::bounded(1);
        let cancel = CancelToken::new();
        let c = cancel.clone();
        let wake = self.loader.waker();
        let label = server.label();
        self.loader.background("dcm-verify", move |_| {
            let res = verify(&server, web.as_ref(), &c);
            let _ = tx.send(Outcome::Verified { server: label, res });
            wake();
        });
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.job = Some(super::Job { rx, cancel });
            d.message = None;
        }
        self.chrome.announce_status(t!("dicomweb-verifying"));
        self.redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_dimse::scp::{ScpConfig, TestScp};

    fn files(n: u32, dir: &Path) -> Vec<PathBuf> {
        let cfg = specular_dicomweb::fake::FakeConfig::synthetic(1, n, 16);
        let st = &cfg.studies[0];
        std::fs::create_dir_all(dir).unwrap();
        (0..n)
            .map(|i| {
                let p = dir.join(format!("IM{i:05}"));
                std::fs::write(&p, specular_dicomweb::fake::part10(st, &st.series[0], i)).unwrap();
                p
            })
            .collect()
    }

    fn run(
        server: Server,
        web: Option<DicomWeb>,
        files: Vec<PathBuf>,
        anonymise: bool,
    ) -> Vec<Event> {
        let (tx, rx) = crossbeam_channel::unbounded();
        Job {
            server,
            web,
            files,
            anonymise,
            cancel: CancelToken::new(),
            tx,
            wake: Arc::new(|| {}),
            registry: Some(net::Registry::new()),
        }
        .run();
        rx.try_iter().collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let d = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/pacs-send"
        ))
        .join(name);
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn failures(ev: &[Event]) -> Vec<&Failed> {
        ev.iter()
            .filter_map(|e| match e {
                Event::One(Err(f)) => Some(f),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn dimse_send_anonymised_with_a_missing_file_named() {
        let dir = scratch("dimse");
        let mut f = files(3, &dir);
        f.insert(1, dir.join("GONE"));
        let scp = TestScp::start(ScpConfig::default()).unwrap();
        let server = Server {
            kind: ServerKind::Dimse,
            host: "127.0.0.1".into(),
            port: scp.port,
            ae_title: "TESTSCP".into(),
            ..Server::default()
        };
        let ev = run(server, None, f, true);
        let ok = ev.iter().filter(|e| matches!(e, Event::One(Ok(_)))).count();
        let failed = failures(&ev);
        assert_eq!(ok, 3);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].name, "GONE");
        assert!(matches!(
            ev.last(),
            Some(Event::End {
                fatal: None,
                cancelled: false
            })
        ));
        let got = scp.received.lock().clone();
        assert_eq!(got.len(), 3);
        for r in &got {
            assert_eq!(r.calling_ae, "SPECULAR");
            // Anonymised: the synthetic patient's name is gone, the UIDs are new.
            let text = String::from_utf8_lossy(&r.data);
            assert!(!text.contains("FAKE^ALPHA"), "patient name sent");
            assert!(r.sop_instance.starts_with("2.25."), "{}", r.sop_instance);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dimse_rejection_is_named() {
        let dir = scratch("reject");
        let f = files(1, &dir);
        let scp = TestScp::start(ScpConfig::default()).unwrap();
        let server = Server {
            kind: ServerKind::Dimse,
            host: "127.0.0.1".into(),
            port: scp.port,
            ae_title: "NOTIT".into(),
            ..Server::default()
        };
        let ev = run(server.clone(), None, f, false);
        match ev.last() {
            Some(Event::End { fatal: Some(m), .. }) => {
                assert!(m.contains("NOTIT") && m.contains("127.0.0.1"), "{m}")
            }
            _ => panic!("no fatal end"),
        }
        let e = verify(&server, None, &CancelToken::new()).unwrap_err();
        assert!(e.contains("NOTIT"), "{e}");
        let mut ok = server;
        ok.ae_title = "TESTSCP".into();
        verify(&ok, None, &CancelToken::new()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stow_send_names_failures() {
        use specular_dicomweb::fake::{FakeConfig, FakeServer};
        let dir = scratch("stow");
        let f = files(3, &dir);
        let bad = specular_dimse::store::head(&std::fs::read(&f[2]).unwrap())
            .unwrap()
            .sop_instance;
        let fake = FakeServer::start(
            FakeConfig {
                stow_fail: vec![bad],
                ..FakeConfig::default()
            },
            specular_remote::testing::ServerOptions::default(),
        )
        .unwrap();
        let client =
            specular_remote::Client::new(specular_remote::ClientConfig::for_tests()).unwrap();
        let web = DicomWeb::new(client, &fake.base(), specular_remote::Auth::None);
        let server = Server {
            base_url: fake.base(),
            ..Server::default()
        };
        let ev = run(server.clone(), Some(web.clone()), f, false);
        let failed = failures(&ev);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].name, "IM00002");
        assert_eq!(fake.stored.lock().len(), 2);
        // One request for all three.
        assert_eq!(fake.posts.load(std::sync::atomic::Ordering::Relaxed), 1);
        verify(&server, Some(&web), &CancelToken::new()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stow_batches_split_on_payload_too_large() {
        use specular_dicomweb::fake::{FakeConfig, FakeServer};
        let dir = scratch("stow-413");
        let mut f = files(5, &dir);
        let one = std::fs::metadata(&f[0]).unwrap().len() as usize;
        // A file too large for the server even alone.
        let big = dir.join("BIG");
        let mut b = std::fs::read(&f[0]).unwrap();
        b.extend(std::iter::repeat_n(0u8, 4 * one));
        std::fs::write(&big, b).unwrap();
        f.push(big);
        let fake = FakeServer::start(
            FakeConfig {
                // Two files per request at most.
                stow_max_body: Some(2 * one + 400),
                ..FakeConfig::default()
            },
            specular_remote::testing::ServerOptions::default(),
        )
        .unwrap();
        let client =
            specular_remote::Client::new(specular_remote::ClientConfig::for_tests()).unwrap();
        let web = DicomWeb::new(client, &fake.base(), specular_remote::Auth::None);
        let server = Server {
            base_url: fake.base(),
            ..Server::default()
        };
        let ev = run(server, Some(web), f, false);
        let failed = failures(&ev);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].name, "BIG");
        assert_eq!(failed[0].reason, t!("pacs-reason-too-large"));
        assert_eq!(fake.stored.lock().len(), 5);
        assert!(matches!(
            ev.last(),
            Some(Event::End {
                fatal: None,
                cancelled: false
            })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
